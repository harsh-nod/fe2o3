use fe2o3_pliron::ProductionRankedTerminatorV1;

/// Inert source-local anchor for a projection argument. Numeric projection
/// identifiers are not source ABI ordinals. The consumer resolves this local
/// through the replayed importer correspondence before inspecting actual O.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionProjectionArgumentComponentV1 {
    /// A scalar source component; its precise origin is independently checked.
    Scalar,
    /// Immutable length metadata of an admitted Slice source formal.
    SliceLength,
}

/// An untrusted projected leaf and the source component it claims to represent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionProjectionArgumentCandidateV1 {
    /// Projection-local Argument or index-producing Local, not an ABI ordinal.
    pub ranked_value: ProductionRankedValueV1,
    /// Local-table identity in the candidate's selected source function.
    pub source_local: SemanticLocalIdV1,
    /// Component independently resolved through admitted importer bindings.
    pub component: ProductionProjectionArgumentComponentV1,
}

/// One source block's contiguous expansion in the existing memory projection.
/// The tail is the source terminator boundary, after all preceding effects.
/// The optional final block is the exact failure trap of a bounds assertion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionProjectionControlBlockV1 {
    /// Source block-table identity, not a canonical O block ordinal.
    pub source_block: SemanticBlockIdV1,
    /// First ranked block emitted for this live source block.
    pub first: u32,
    /// Ranked block containing the source terminator boundary.
    pub tail: u32,
    /// Exclusive end of this source block's ranked expansion.
    pub end: u32,
}

/// Claims recorded by the existing CFG emitter, not an executable graph or a
/// proof. Public construction cannot create the completing scoped capability.
#[derive(Debug, Default)]
pub struct ProductionProjectionControlCandidateV1 {
    /// Source-ordered live block segments; omissions are checked independently.
    pub blocks: Vec<ProductionProjectionControlBlockV1>,
    /// Inert leaf anchors recorded before the emitter's local maps are dropped.
    pub arguments: Vec<ProductionProjectionArgumentCandidateV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceOutputProjectionArgumentV1 {
    ranked_value: ProductionRankedValueV1,
    source_local: SemanticLocalIdV1,
    component: ProductionProjectionArgumentComponentV1,
    scalar: ProductionSemanticScalarTypeV2,
    origin: SourceOutputProjectionLeafOriginV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceOutputProjectionLeafOriginV1 {
    Formal(SourceOutputProjectionFormalV1),
    Literal(SourceOutputProjectionLiteralV1),
    Invocation(SourceOutputProjectionInvocationV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceOutputProjectionInvocationV1 {
    source_index: usize,
    source: SourceOutputInvocationSourceAnchorV1,
    original: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    output: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    symbol: u32,
    first_use: usize,
    end_use: usize,
    // Private address substitution also requires the candidate's retained
    // own-Store seal. Public leaf queries remain closed; full-ranked joins
    // independently authenticate the same source chain and their own claims.
    identity_getter: bool,
}

enum SourceOutputAddressLeafV1<'scope> {
    Formal(ProductionConditionalMemoryArgumentV1<'scope>),
    Literal(ProductionConditionalMemoryLiteralV1<'scope>),
    Invocation(
        &'scope SourceOutputProjectionArgumentV1,
        &'scope SourceOutputProjectionInvocationV1,
    ),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceOutputProjectionFormalV1 {
    original: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    output: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    output_value: ValueId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceOutputProjectionLiteralV1 {
    block: SemanticBlockIdV1,
    statement: u32,
    original: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    value: ValueId,
    ssa: SsaValueV1,
    bits: u64,
    first_use: usize,
    end_use: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceOutputProjectionLiteralUseV1 {
    guard: SemanticBlockIdV1,
    input: SourceOutputControlUseIdentityV1,
    output: SourceOutputControlUseIdentityV1,
}

impl SourceOutputProjectionArgumentV1 {
    fn original(&self) -> fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 {
        match self.origin {
            SourceOutputProjectionLeafOriginV1::Formal(formal) => formal.original,
            SourceOutputProjectionLeafOriginV1::Literal(literal) => literal.original,
            SourceOutputProjectionLeafOriginV1::Invocation(invocation) => invocation.original,
        }
    }
}

struct SourceOutputControlCandidateIdentityV1<'a> {
    candidate_identity: *const (),
    root: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    canonical: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    lowering: &'a ProductionRankedKernelLoweringInputV1,
    sources: &'a [ProductionRankedAccessSourceV1],
    effects: &'a [ProductionRankedExecutableEffectSourceV1],
    claims: &'a ProductionProjectionControlCandidateV1,
    arguments: std::ops::Range<usize>,
    identity_address: Option<SourceOutputIdentityAddressV1>,
}

/// Conditional memory-path coverage of exact borrowed inputs. Actual O is the
/// only executable. In particular, a retained Call continuation is conditional
/// on that Call returning; this capability proves neither termination nor
/// progress/convergence nor equality of feasible paths. It cannot authorize
/// native admission or replace an address/value/formal-memory check.
///
/// No constructor, Clone, serialization, owned receipt or raw lowering getter
/// is exposed. A later address check must consume these SAME checked argument
/// anchors, not choose another renaming for equal-typed source parameters.
/// The address/floor checks below enforce the documented caller ledger
/// precondition; they are not a replacement-resistant ledger identity. A caller
/// must not replace a Budget in place, reset history, or release live owners.
pub struct ProductionConditionalMemoryControlCoverageV1<'scope> {
    view_identity: *const (),
    budget_identity: *const (),
    live_floor: usize,
    output: &'scope fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    inventory: &'scope fe2o3_kernel_analysis::CanonicalKirInventoryV1<'scope>,
    candidates: &'scope [SourceOutputControlCandidateIdentityV1<'scope>],
    arguments: &'scope [SourceOutputProjectionArgumentV1],
    literal_uses: &'scope [SourceOutputProjectionLiteralUseV1],
    invocation_sources: &'scope [SourceOutputInvocationSourceIndexV1],
    invocation_roots: &'scope [(fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1, u32)],
}

/// A borrow of the same source-anchored substitution used for control checks.
/// Numeric coordinates are inert when copied; only the enclosing scoped
/// relation connects them to this exact source/output/candidate combination.
pub struct ProductionConditionalMemoryArgumentV1<'scope> {
    row: &'scope SourceOutputProjectionArgumentV1,
    formal: &'scope SourceOutputProjectionFormalV1,
}

impl ProductionConditionalMemoryArgumentV1<'_> {
    /// Returns the exact admitted source local for this component.
    pub const fn source_local(&self) -> SemanticLocalIdV1 {
        self.row.source_local
    }
    /// Distinguishes a scalar formal from metadata of a Slice formal.
    pub const fn component(&self) -> ProductionProjectionArgumentComponentV1 {
        self.row.component
    }
    /// The original formal anchor. SliceLength identifies the Slice formal,
    /// not a scalar-valued extraction; its component must be interpreted too.
    pub const fn original(&self) -> fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 {
        self.formal.original
    }
    /// The checked output formal anchor, with the same component distinction.
    pub const fn output(&self) -> fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 {
        self.formal.output
    }
    /// Value of the output formal anchor. For SliceLength this is a Slice,
    /// never a U64 operand. Consumers must check the actual typed extraction.
    pub const fn output_value(&self) -> ValueId {
        self.formal.output_value
    }
    /// Type of the represented scalar component, not necessarily its anchor.
    pub const fn scalar(&self) -> ProductionSemanticScalarTypeV2 {
        self.row.scalar
    }
}

/// A borrowed leaf in the one checked projection substitution. Neither variant
/// authorizes an address occurrence, formal discharge, or native admission.
pub enum ProductionConditionalMemoryIndexLeafV1<'scope> {
    /// A source formal or its immutable SliceLength component.
    Formal(ProductionConditionalMemoryArgumentV1<'scope>),
    /// A single source definition with individually checked bounds-guard uses.
    Literal(ProductionConditionalMemoryLiteralV1<'scope>),
}

/// Borrowed source literal and exact control-use witnesses. The uses below are
/// Compare operands, not interchangeable physical address/index occurrences.
pub struct ProductionConditionalMemoryLiteralV1<'scope> {
    row: &'scope SourceOutputProjectionArgumentV1,
    literal: &'scope SourceOutputProjectionLiteralV1,
    uses: &'scope [SourceOutputProjectionLiteralUseV1],
}

impl ProductionConditionalMemoryLiteralV1<'_> {
    /// Exact local-table identity in the selected source function.
    pub const fn source_local(&self) -> SemanticLocalIdV1 {
        self.row.source_local
    }
    /// Source block containing the sole literal definition.
    pub const fn source_block(&self) -> SemanticBlockIdV1 {
        self.literal.block
    }
    /// Exact source statement ordinal, not an SSA event ordinal.
    pub const fn source_statement(&self) -> u32 {
        self.literal.statement
    }
    /// Original N Constant definition authenticated by the statement span.
    pub const fn original(&self) -> fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 {
        self.literal.original
    }
    /// Preserved unsigned or Boolean scalar type.
    pub const fn scalar(&self) -> ProductionSemanticScalarTypeV2 {
        self.row.scalar
    }
    /// Exact bits after checked source type/size/range interpretation.
    pub const fn bits(&self) -> u64 {
        self.literal.bits
    }
    /// Number of separately checked guard-use occurrences, not proof by count.
    pub fn guard_use_count(&self) -> usize {
        self.uses.len()
    }
    /// Borrows one already checked guard occurrence; copied coordinates alone
    /// do not carry the scoped source/output relation.
    pub fn guard_use(&self, ordinal: usize) -> Option<ProductionConditionalMemoryLiteralUseV1<'_>> {
        self.uses
            .get(ordinal)
            .map(|row| ProductionConditionalMemoryLiteralUseV1 { row })
    }
}

/// Exact original/checked-output operand association at one bounds guard.
pub struct ProductionConditionalMemoryLiteralUseV1<'scope> {
    row: &'scope SourceOutputProjectionLiteralUseV1,
}
impl ProductionConditionalMemoryLiteralUseV1<'_> {
    /// Source block whose Assert consumes this literal definition.
    pub const fn source_guard(&self) -> SemanticBlockIdV1 {
        self.row.guard
    }
    /// Exact original N Compare operand occurrence.
    pub const fn original_use(&self) -> fe2o3_kernel_ir::CanonicalKirUseCoordinateV1 {
        self.row.input.coordinate
    }
    /// Definition consumed by the original Compare, possibly a same-width cast.
    pub const fn original_definition(&self) -> fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 {
        self.row.input.definition
    }
    /// Actual retained O Compare operand occurrence. Absence is never invented.
    pub const fn output_use(&self) -> fe2o3_kernel_ir::CanonicalKirUseCoordinateV1 {
        self.row.output.coordinate
    }
    /// Actual definition consumed by that exact O operand.
    pub const fn output_definition(&self) -> fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 {
        self.row.output.definition
    }
    /// Actual value consumed at the checked O occurrence.
    pub const fn output_value(&self) -> ValueId {
        self.row.output.value
    }
}

impl ProductionConditionalMemoryControlCoverageV1<'_> {
    /// Borrows actual O without granting formal-memory or native admission.
    pub fn output(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        self.output
    }

    /// Looks up a formal component in the same map used by control normalization.
    /// Non-formal rows return None; index_leaf separately exposes literals.
    /// Revalidates exact candidate custody and caller-owned accounting first;
    /// an absent row grants no meaning to an opaque ranked value.
    pub fn index_value(
        &self,
        view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
        candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
        ranked_value: ProductionRankedValueV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Option<ProductionConditionalMemoryArgumentV1<'_>>, ProductionSourceOutputErrorV1>
    {
        Ok(
            match self.index_leaf(view, candidate, ranked_value, budget)? {
                Some(ProductionConditionalMemoryIndexLeafV1::Formal(value)) => Some(value),
                Some(ProductionConditionalMemoryIndexLeafV1::Literal(_)) | None => None,
            },
        )
    }

    /// Borrows a formal or a typed literal from the SAME checked leaf map.
    /// Literal records cover only their exact listed guard uses; a later
    /// address/extent relation must check its own actual O operand occurrence.
    /// Private invocation rows intentionally return None through this API.
    pub fn index_leaf(
        &self,
        view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
        candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
        ranked_value: ProductionRankedValueV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Option<ProductionConditionalMemoryIndexLeafV1<'_>>, ProductionSourceOutputErrorV1>
    {
        let ordinal = self.candidate_ordinal_v1(view, candidate, budget)?;
        Ok(source_output_control_leaf_rows_v1(
            self.arguments,
            self.literal_uses,
            self.candidates[ordinal].arguments.clone(),
            ranked_value,
            budget,
        )?
        .and_then(|leaf| match leaf {
            SourceOutputAddressLeafV1::Formal(value) => {
                Some(ProductionConditionalMemoryIndexLeafV1::Formal(value))
            }
            SourceOutputAddressLeafV1::Literal(value) => {
                Some(ProductionConditionalMemoryIndexLeafV1::Literal(value))
            }
            SourceOutputAddressLeafV1::Invocation(..) => None,
        }))
    }

    fn address_leaf_v1(
        &self,
        view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
        candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
        ordinal: usize,
        ranked_value: ProductionRankedValueV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Option<SourceOutputAddressLeafV1<'_>>, ProductionSourceOutputErrorV1> {
        self.require_candidate_at_v1(view, candidate, ordinal, budget)?;
        if let Some(identity) = &self.candidates[ordinal].identity_address
            && identity.ranked_index == ranked_value
        {
            use ProductionSourceOutputErrorV1 as Error;
            budget.charge_work(12).map_err(Error::Resource)?;
            let arguments = self
                .arguments
                .get(self.candidates[ordinal].arguments.clone())
                .ok_or(Error::Invalid("identity address argument span absent"))?;
            let found = assert_origin_find_v1(arguments, budget, |row, budget| {
                budget.charge_work(1)?;
                Ok(row.ranked_value.cmp(&ranked_value))
            })
            .map_err(Error::SourceOrigin)?
            .ok_or(Error::Invalid("identity address index anchor absent"))?;
            let row = &arguments[found];
            let SourceOutputProjectionLeafOriginV1::Invocation(invocation) = &row.origin else {
                return Err(Error::Invalid("identity address index is not Invocation"));
            };
            if !invocation.identity_getter
                || *invocation != identity.invocation
                || row.source_local != identity.witness
                || row.component != ProductionProjectionArgumentComponentV1::Scalar
                || row.scalar != source_output_address_u64_v1()
            {
                return Err(Error::Invalid("identity address sealed index differs"));
            }
            return Ok(Some(SourceOutputAddressLeafV1::Invocation(row, invocation)));
        }
        source_output_control_leaf_rows_v1(
            self.arguments,
            self.literal_uses,
            self.candidates[ordinal].arguments.clone(),
            ranked_value,
            budget,
        )
    }

    fn identity_address_use_v1(
        &self,
        view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
        candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
        ordinal: usize,
        source: ProductionRankedAccessSourceV1,
        operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<
        Option<(
            &SourceOutputIdentityAddressV1,
            &SourceOutputIdentityStoreAddressV1,
        )>,
        ProductionSourceOutputErrorV1,
    > {
        use ProductionSourceOutputErrorV1 as Error;
        self.require_candidate_at_v1(view, candidate, ordinal, budget)?;
        budget.charge_work(16).map_err(Error::Resource)?;
        let Some(identity) = &self.candidates[ordinal].identity_address else {
            return Ok(None);
        };
        let statement = source
            .semantic_statement()
            .ok_or(Error::Invalid("identity address source statement absent"))?;
        if source.semantic_access_ordinal() != 0 {
            return Err(Error::Invalid(
                "identity address source access ordinal differs",
            ));
        }
        let key = (source.semantic_block(), statement);
        let found = assert_origin_find_v1(&identity.stores, budget, |row, budget| {
            budget.charge_work(3)?;
            Ok(row.site.cmp(&key))
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("identity address own source Store absent"))?;
        let row = &identity.stores[found];
        let address = row
            .address
            .as_ref()
            .ok_or(Error::Invalid("identity address own Store seal absent"))?;
        if !row.seen
            || address.output != operation
            || address.original.block.function != self.candidates[ordinal].canonical
            || address.output.block.function != self.candidates[ordinal].canonical
            || address.pointer.coordinate
                != (fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
                    operation,
                    operand: 0,
                })
        {
            return Err(Error::Invalid(
                "identity address own Store occurrence differs",
            ));
        }
        let original = view
            .source
            .executable()
            .module()
            .functions
            .get(address.original.block.function.0 as usize)
            .and_then(|function| function.body.as_ref())
            .and_then(|body| body.blocks.get(address.original.block.block as usize))
            .and_then(|block| block.operations.get(address.original.operation as usize))
            .ok_or(Error::Invalid("identity address original Store absent"))?;
        if !matches!(original.kind, OperationKind::Store { pointer, .. } if pointer == identity.original_pointer)
        {
            return Err(Error::Invalid(
                "identity address original getter pointer differs",
            ));
        }
        Ok(Some((identity, address)))
    }

    fn address_inventory_v1(
        &self,
        view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
        candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
        ordinal: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<&fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>, ProductionSourceOutputErrorV1>
    {
        self.require_candidate_at_v1(view, candidate, ordinal, budget)?;
        budget
            .charge_work(1)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        Ok(self.inventory)
    }

    fn candidate_ordinal_v1(
        &self,
        view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
        candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<usize, ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        for (ordinal, row) in self.candidates.iter().enumerate() {
            budget.charge_work(2).map_err(Error::Resource)?;
            if row.root == candidate.selected_root && row.function == candidate.selected_function {
                self.require_candidate_at_v1(view, candidate, ordinal, budget)?;
                return Ok(ordinal);
            }
        }
        Err(Error::Invalid("conditional control root absent"))
    }

    fn require_candidate_at_v1(
        &self,
        view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
        candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
        ordinal: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<(), ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        budget.charge_work(14).map_err(Error::Resource)?;
        if self.budget_identity != std::ptr::from_ref(budget).cast::<()>()
            || budget.storage() < self.live_floor
        {
            return Err(Error::Resource(AssertOriginResourceV1::Accounting));
        }
        if self.view_identity != std::ptr::from_ref(view).cast::<()>()
            || !std::ptr::eq(self.output, view.output())
        {
            return Err(Error::Invalid("conditional control owner differs"));
        }
        let row = self.candidates.get(ordinal).ok_or(Error::Invalid(
            "conditional control candidate ordinal absent",
        ))?;
        if row.root != candidate.selected_root || row.function != candidate.selected_function {
            return Err(Error::Invalid(
                "conditional control candidate ordinal differs",
            ));
        }
        if !std::ptr::eq(row.lowering, candidate.lowering)
            || row.candidate_identity != std::ptr::from_ref(candidate).cast::<()>()
            || !std::ptr::eq(row.sources, candidate.access_sources)
            || !std::ptr::eq(row.effects, candidate.executable_effect_sources)
            || !std::ptr::eq(row.claims, candidate.control)
        {
            return Err(Error::Invalid("conditional control candidate differs"));
        }
        Ok(())
    }

    fn output_function_at_v1(
        &self,
        view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
        candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
        ordinal: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1, ProductionSourceOutputErrorV1>
    {
        self.require_candidate_at_v1(view, candidate, ordinal, budget)?;
        budget
            .charge_work(1)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        Ok(self.candidates[ordinal].canonical)
    }
}

fn source_output_control_argument_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
    canonical: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    claim: ProductionProjectionArgumentCandidateV1,
    literal_uses: &mut Vec<SourceOutputProjectionLiteralUseV1>,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputProjectionArgumentV1, ProductionSourceOutputErrorV1> {
    use ProductionProjectionArgumentComponentV1 as Component;
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(8).map_err(Error::Resource)?;
    let source = view.source.semantic_ssa().source_semantic();
    let function = source
        .functions()
        .get(candidate.selected_function.index() as usize)
        .ok_or(Error::Invalid("control source function absent"))?;
    let local = function
        .locals()
        .get(claim.source_local.index() as usize)
        .ok_or(Error::Invalid("control argument source local absent"))?;
    if !matches!(local.role(), SemanticLocalRoleV1::Argument(_)) {
        return source_output_control_literal_v1(
            view,
            candidate,
            canonical,
            claim,
            literal_uses,
            inventory,
            budget,
        );
    }
    for block in function.blocks() {
        budget.charge_work(2).map_err(Error::Resource)?;
        for statement in block.statements() {
            budget.charge_work(3).map_err(Error::Resource)?;
            if let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                && assignment.destination().local() == claim.source_local
                && assignment.destination().projections().is_empty()
            {
                return Err(Error::Invalid("control source formal is redefined"));
            }
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && call.destination().is_some_and(|destination| {
                destination.place().local() == claim.source_local
                    && destination.place().projections().is_empty()
            })
        {
            return Err(Error::Invalid("control source formal is redefined by Call"));
        }
    }
    let original = view
        .source
        .executable()
        .module()
        .functions
        .get(canonical.0 as usize)
        .ok_or(Error::Invalid("control original function absent"))?;
    let output = view.source_output_exact_entry_v1(
        candidate.selected_root,
        candidate.selected_function,
        budget,
    )?;
    let original_body = original
        .body
        .as_ref()
        .ok_or(Error::Invalid("control original body absent"))?;
    let output_body = output
        .body
        .as_ref()
        .ok_or(Error::Invalid("control output body absent"))?;
    let mut source_value = None;
    for binding in view.source.correspondence.parameter_bindings() {
        budget.charge_work(4).map_err(Error::Resource)?;
        if binding.correspondence_owner() == candidate.selected_root
            && binding.semantic_function() == candidate.selected_function
            && binding.semantic_local() == claim.source_local
            && source_value.replace(binding.kernel_ir_value()).is_some()
        {
            return Err(Error::Invalid(
                "control argument source binding is ambiguous",
            ));
        }
    }
    let source_value = source_value.ok_or(Error::Invalid(
        "control source formal has no direct importer binding",
    ))?;
    let mut ordinal = None;
    for (index, value) in original_body.parameters.iter().enumerate() {
        budget.charge_work(2).map_err(Error::Resource)?;
        if *value == source_value && ordinal.replace(index).is_some() {
            return Err(Error::Invalid(
                "control source formal has duplicate parameter values",
            ));
        }
    }
    budget.charge_work(6).map_err(Error::Resource)?;
    let ordinal = ordinal.ok_or(Error::Invalid("control source formal parameter absent"))?;
    let ty = original
        .signature
        .parameters
        .get(ordinal)
        .ok_or(Error::Invalid("control source parameter type absent"))?;
    if output.signature.parameters.get(ordinal) != Some(ty) {
        return Err(Error::Invalid("control output parameter type differs"));
    }
    let scalar = match claim.component {
        Component::Scalar => {
            kir_semantic_scalar_v1(ty).ok_or(Error::Invalid("control formal is not scalar"))?
        }
        Component::SliceLength => {
            if !matches!(ty, Type::Slice(_)) {
                return Err(Error::Invalid("control metadata formal is not a slice"));
            }
            ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 64,
            }
        }
    };
    let output_value = *output_body
        .parameters
        .get(ordinal)
        .ok_or(Error::Invalid("control output formal value absent"))?;
    let argument =
        u32::try_from(ordinal).map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    // The independently checked transition preserves each exact signature slot;
    // this is not a candidate-selected substitution among equal-typed formals.
    Ok(SourceOutputProjectionArgumentV1 {
        ranked_value: claim.ranked_value,
        source_local: claim.source_local,
        component: claim.component,
        origin: SourceOutputProjectionLeafOriginV1::Formal(SourceOutputProjectionFormalV1 {
            original: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::FunctionArgument {
                function: canonical,
                argument,
            },
            output: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::FunctionArgument {
                function: canonical,
                argument,
            },
            output_value,
        }),
        scalar,
    })
}

fn source_output_control_literal_constant_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    constant: &fe2o3_mir_model::semantic_mir_v1::SemanticConstantV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(Type, Constant, ProductionSemanticScalarTypeV2, u64), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(8).map_err(Error::Resource)?;
    let shape = types
        .get(ty.index() as usize)
        .map(SemanticTypeDeclV1::shape);
    if constant.ty() != ty
        || !matches!(
            shape,
            Some(SemanticTypeShapeV1::Scalar(
                SemanticScalarTypeV1::Bool
                    | SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 8 | 16 | 32 | 64
                    }
            ))
        )
    {
        return Err(Error::Invalid("literal source type unsupported"));
    }
    let SemanticConstantValueV1::Scalar(value) = constant.value() else {
        return Err(Error::Invalid("literal source payload is not scalar"));
    };
    let ty = lower_scalar_type(types, ty).map_err(Error::SourceReplay)?;
    let scalar = kir_semantic_scalar_v1(&ty).ok_or(Error::Invalid("literal scalar absent"))?;
    let width = source_output_control_unsigned_width_v1(scalar)
        .ok_or(Error::Invalid("literal scalar width unsupported"))?;
    if value.size_bytes() != width.div_ceil(8) as u8
        || value.bits() > u128::from(u64::MAX)
        || (width < 64 && value.bits() >= (1u128 << width))
    {
        return Err(Error::Invalid("literal source size or bits differ"));
    }
    let lowered = lower_constant(ty.clone(), *value).map_err(Error::SourceReplay)?;
    let normalized = normalize_kir_constant_v1(&lowered)
        .ok_or(Error::Invalid("literal normalization unavailable"))?;
    if normalized != (scalar, value.bits() as u64) {
        return Err(Error::Invalid("literal normalized type or bits differ"));
    }
    Ok((ty, lowered, scalar, normalized.1))
}

fn source_output_control_literal_definition_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
    canonical: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    claim: ProductionProjectionArgumentCandidateV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<
    (
        SourceOutputProjectionLiteralV1,
        ProductionSemanticScalarTypeV2,
    ),
    ProductionSourceOutputErrorV1,
> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::{
        CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Def,
        CanonicalKirOperationCoordinateV1 as Op,
    };
    budget.charge_work(7).map_err(Error::Resource)?;
    let source = view.source.semantic_ssa().source_semantic();
    let function = source
        .functions()
        .get(candidate.selected_function.index() as usize)
        .ok_or(Error::Invalid("literal source function absent"))?;
    let local = function
        .locals()
        .get(claim.source_local.index() as usize)
        .ok_or(Error::Invalid("literal local absent"))?;
    if local.role() != SemanticLocalRoleV1::Temporary
        || claim.component != ProductionProjectionArgumentComponentV1::Scalar
    {
        return Err(Error::Invalid("literal requires a plain temporary scalar"));
    }
    let mut assignment = None;
    for (block_index, block) in function.blocks().iter().enumerate() {
        budget.charge_work(2).map_err(Error::Resource)?;
        for (statement_index, statement) in block.statements().iter().enumerate() {
            budget.charge_work(3).map_err(Error::Resource)?;
            match statement.kind() {
                SemanticStatementKindV1::Assign(value)
                    if value.destination().local() == claim.source_local =>
                {
                    let SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(constant)) =
                        value.value().kind()
                    else {
                        return Err(Error::Invalid(
                            "literal source is not a direct constant assignment",
                        ));
                    };
                    if !value.destination().projections().is_empty()
                        || value.destination().ty() != local.ty()
                        || value.value().result_type() != local.ty()
                        || assignment
                            .replace((block_index, statement_index, constant))
                            .is_some()
                    {
                        return Err(Error::Invalid(
                            "literal source definition is projected, redefined or mistyped",
                        ));
                    }
                }
                SemanticStatementKindV1::StorageDead(id) if *id == claim.source_local => {
                    return Err(Error::Invalid("literal source lifetime has a kill"));
                }
                SemanticStatementKindV1::Deinitialize(place)
                | SemanticStatementKindV1::SetDiscriminant { place, .. }
                    if place.local() == claim.source_local =>
                {
                    return Err(Error::Invalid("literal source is mutated"));
                }
                _ => {}
            }
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && call
                .destination()
                .is_some_and(|to| to.place().local() == claim.source_local)
        {
            return Err(Error::Invalid("literal source has a Call definition"));
        }
    }
    let (block, statement, constant) =
        assignment.ok_or(Error::Invalid("literal source assignment absent"))?;
    let (ty, constant, scalar, bits) =
        source_output_control_literal_constant_v1(source.types(), local.ty(), constant, budget)?;
    budget.charge_work(3).map_err(Error::Resource)?;
    let plan = view
        .source
        .semantic_ssa()
        .plan_for_function(candidate.selected_function)
        .ok_or(Error::Invalid("literal SSA plan absent"))?
        .plan();
    let variable = fe2o3_mir_model::SsaVariableIdV1::new(claim.source_local.index());
    budget
        .charge_work(plan.promoted_variables().len())
        .map_err(Error::Resource)?;
    if !plan.promoted_variables().contains(&variable) {
        return Err(Error::Invalid(
            "literal source is retained or address escaped",
        ));
    }
    let mut definition = None;
    for index in 0..function.blocks().len() {
        budget.charge_work(1).map_err(Error::Resource)?;
        for (_, event) in plan
            .resolved_events(SsaBlockIdV1::new(index as u32))
            .unwrap_or(&[])
        {
            budget.charge_work(2).map_err(Error::Resource)?;
            match *event {
                SsaResolvedEventV1::Define {
                    variable: found,
                    value,
                } if found == variable
                    && (index != block
                        || !matches!(value, SsaValueV1::Definition(_))
                        || definition.replace(value).is_some()) =>
                {
                    return Err(Error::Invalid("literal SSA definition differs"));
                }
                SsaResolvedEventV1::Kill {
                    variable: found, ..
                } if found == variable => {
                    return Err(Error::Invalid("literal SSA definition is killed"));
                }
                _ => {}
            }
        }
    }
    let ssa = definition.ok_or(Error::Invalid("literal SSA definition absent"))?;
    let block = SemanticBlockIdV1::from_index(
        u32::try_from(block).map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
    );
    let statement = u32::try_from(statement)
        .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    let mut span = None;
    for row in view.source.correspondence.statement_operation_spans() {
        budget.charge_work(5).map_err(Error::Resource)?;
        if row.correspondence_owner() == candidate.selected_root
            && row.semantic_function() == candidate.selected_function
            && row.semantic_block() == block
            && row.statement_ordinal() == statement
            && span.replace(*row).is_some()
        {
            return Err(Error::Invalid("literal statement span ambiguous"));
        }
    }
    budget.charge_work(4).map_err(Error::Resource)?;
    let span = span.ok_or(Error::Invalid("literal statement span absent"))?;
    if span.operation_count() != 1 {
        return Err(Error::Invalid("literal statement is not one Constant"));
    }
    let body = view
        .source
        .executable()
        .module()
        .functions
        .get(canonical.0 as usize)
        .and_then(|function| function.body.as_ref())
        .ok_or(Error::Invalid("literal N body absent"))?;
    let mut found = None;
    for (block_index, actual) in body.blocks.iter().enumerate() {
        budget.charge_work(2).map_err(Error::Resource)?;
        if actual.id == span.kernel_ir_block() {
            let operation = actual
                .operations
                .get(span.first_operation_ordinal() as usize)
                .ok_or(Error::Invalid("literal N operation absent"))?;
            budget.charge_work(5).map_err(Error::Resource)?;
            if operation.results.len() != 1
                || operation.results[0].ty != ty
                || !matches!(&operation.kind, OperationKind::Constant(value) if *value == constant)
            {
                return Err(Error::Invalid("literal N Constant type or payload differs"));
            }
            let coordinate = Def::Result {
                operation: Op {
                    block: Block {
                        function: canonical,
                        block: block_index as u32,
                    },
                    operation: span.first_operation_ordinal(),
                },
                result: 0,
            };
            if found
                .replace((coordinate, operation.results[0].id))
                .is_some()
            {
                return Err(Error::Invalid("literal N block duplicated"));
            }
        }
    }
    let (original, value) = found.ok_or(Error::Invalid("literal N block absent"))?;
    Ok((
        SourceOutputProjectionLiteralV1 {
            block,
            statement,
            original,
            value,
            ssa,
            bits,
            first_use: 0,
            end_use: 0,
        },
        scalar,
    ))
}

// This is ancestry validation, not constant evaluation. Only an exact source
// Constant and same-scalar Bitcast links may connect the original guard use.
fn source_output_control_literal_ancestry_v1(
    body: &fe2o3_kernel_ir::FunctionBody,
    canonical: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    mut value: ValueId,
    literal: SourceOutputProjectionLiteralV1,
    scalar: ProductionSemanticScalarTypeV2,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    for _ in 0..=MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        budget.charge_work(1).map_err(Error::Resource)?;
        if value == literal.value {
            budget.charge_work(4).map_err(Error::Resource)?;
            let fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result {
                operation,
                result: 0,
            } = literal.original
            else {
                return Err(Error::Invalid("literal original result slot differs"));
            };
            if operation.block.function != canonical {
                return Err(Error::Invalid("literal original function differs"));
            }
            let operation = body
                .blocks
                .get(operation.block.block as usize)
                .and_then(|block| block.operations.get(operation.operation as usize))
                .ok_or(Error::Invalid("literal original definition absent"))?;
            if operation.results.len() != 1
                || operation.results[0].id != literal.value
                || kir_semantic_scalar_v1(&operation.results[0].ty) != Some(scalar)
                || !matches!(&operation.kind, OperationKind::Constant(constant)
                    if normalize_kir_constant_v1(constant) == Some((scalar, literal.bits)))
            {
                return Err(Error::Invalid(
                    "literal original definition, type or bits differ",
                ));
            }
            return Ok(());
        }
        let mut next = None;
        for block in &body.blocks {
            budget.charge_work(1).map_err(Error::Resource)?;
            for operation in &block.operations {
                budget
                    .charge_work(
                        operation
                            .results
                            .len()
                            .checked_add(2)
                            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                    )
                    .map_err(Error::Resource)?;
                if !operation.results.iter().any(|result| result.id == value) {
                    continue;
                }
                let OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value: operand,
                    to,
                } = &operation.kind
                else {
                    return Err(Error::Invalid(
                        "literal N ancestry is not same-scalar transport",
                    ));
                };
                if operation.results.len() != 1
                    || operation.results[0].id != value
                    || kir_semantic_scalar_v1(&operation.results[0].ty) != Some(scalar)
                    || kir_semantic_scalar_v1(to) != Some(scalar)
                    || next.replace(*operand).is_some()
                {
                    return Err(Error::Invalid("literal N cast type or definition differs"));
                }
            }
        }
        value = next.ok_or(Error::Invalid("literal N ancestry definition absent"))?;
    }
    Err(Error::Invalid("literal N ancestry depth exceeded"))
}

fn source_output_control_literal_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
    canonical: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    claim: ProductionProjectionArgumentCandidateV1,
    uses: &mut Vec<SourceOutputProjectionLiteralUseV1>,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputProjectionArgumentV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::{
        CanonicalKirDefinitionCoordinateV1 as Def, CanonicalKirUseCoordinateV1 as Use,
    };
    let (mut literal, scalar) =
        source_output_control_literal_definition_v1(view, candidate, canonical, claim, budget)?;
    budget.charge_work(4).map_err(Error::Resource)?;
    let source = view.source.semantic_ssa().source_semantic();
    let function = &source.functions()[candidate.selected_function.index() as usize];
    let body = view.source.executable().module().functions[canonical.0 as usize]
        .body
        .as_ref()
        .ok_or(Error::Invalid("literal original body absent"))?;
    let plan = view
        .source
        .semantic_ssa()
        .plan_for_function(candidate.selected_function)
        .ok_or(Error::Invalid("literal SSA plan absent"))?
        .plan();
    literal.first_use = uses.len();
    for segment in &candidate.control.blocks {
        budget.charge_work(4).map_err(Error::Resource)?;
        let tail = candidate
            .lowering
            .kernel()
            .blocks()
            .get(segment.tail as usize)
            .ok_or(Error::Invalid("literal projected guard absent"))?;
        let (lhs, rhs) = match tail.terminator() {
            ProductionRankedTerminatorV1::IndexLessThan { lhs, rhs, .. }
            | ProductionRankedTerminatorV1::IndexEqual { lhs, rhs, .. } => (*lhs, *rhs),
            _ => continue,
        };
        if lhs != claim.ranked_value && rhs != claim.ranked_value {
            continue;
        }
        if lhs != claim.ranked_value || rhs == claim.ranked_value {
            return Err(Error::Invalid(
                "literal requires the exact bounds index operand",
            ));
        }
        let block = function
            .blocks()
            .get(segment.source_block.index() as usize)
            .ok_or(Error::Invalid("literal source guard absent"))?;
        let SemanticTerminatorKindV1::Assert {
            message: SemanticAssertMessageV1::BoundsCheck { index, .. },
            ..
        } = block.terminator().kind()
        else {
            return Err(Error::Invalid(
                "literal projected use is not a source bounds guard",
            ));
        };
        let SemanticOperandV1::Copy(place) = index else {
            return Err(Error::Invalid(
                "literal bounds index is not a copied plain local",
            ));
        };
        if place.local() != claim.source_local
            || !place.projections().is_empty()
            || place.ty() != function.locals()[claim.source_local.index() as usize].ty()
        {
            return Err(Error::Invalid("literal source guard local or type differs"));
        }
        let mut count = 0usize;
        for (_, event) in plan
            .resolved_events(SsaBlockIdV1::new(segment.source_block.index()))
            .ok_or(Error::Invalid("literal source guard SSA events absent"))?
        {
            budget.charge_work(2).map_err(Error::Resource)?;
            if let SsaResolvedEventV1::Use { variable, value } = *event
                && variable.get() == claim.source_local.index()
            {
                if value != literal.ssa {
                    return Err(Error::Invalid(
                        "literal guard uses a different SSA definition",
                    ));
                }
                count += 1;
            }
        }
        if count == 0 {
            return Err(Error::Invalid("literal guard SSA use absent"));
        }
        let assertion = view
            .source
            .assert_origins()
            .assert_condition(
                candidate.selected_root,
                candidate.selected_function,
                segment.source_block,
                budget,
            )
            .map_err(Error::SourceOrigin)?;
        let SemanticKirAssertConditionOutcomeV1::Emitted {
            condition_use,
            definition,
            ..
        } = assertion.outcome()
        else {
            return Err(Error::Invalid("literal assertion condition was elided"));
        };
        let Def::Result {
            operation,
            result: 0,
        } = definition
        else {
            return Err(Error::Invalid(
                "literal guard condition is not a direct Compare",
            ));
        };
        if operation.block.function != canonical
            || !matches!(condition_use,
            Use::TerminatorOperand { block, operand: 0 } if block == operation.block)
        {
            return Err(Error::Invalid("literal assertion occurrence differs"));
        }
        budget.charge_work(5).map_err(Error::Resource)?;
        let compare = body
            .blocks
            .get(operation.block.block as usize)
            .and_then(|block| block.operations.get(operation.operation as usize))
            .ok_or(Error::Invalid("literal original Compare absent"))?;
        let OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: original_value,
            ..
        } = &compare.kind
        else {
            return Err(Error::Invalid(
                "literal original guard is not a less-than Compare",
            ));
        };
        source_output_control_literal_ancestry_v1(
            body,
            canonical,
            *original_value,
            literal,
            scalar,
            budget,
        )?;
        let coordinate = Use::OperationOperand {
            operation,
            operand: 0,
        };
        let index = assert_origin_find_v1(
            &view.checked_control_rows.compare_uses,
            budget,
            |row, budget| {
                budget.charge_work(1)?;
                Ok(row.input.coordinate.cmp(&coordinate))
            },
        )
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("literal checked Compare use absent"))?;
        let row = view.checked_control_rows.compare_uses[index];
        let output =
            source_output_control_literal_row_v1(row, coordinate, *original_value, budget)?;
        let actual = source_output_control_use_identity_v1(inventory, output.coordinate, budget)?;
        if actual != output {
            return Err(Error::Invalid("literal mapped Compare use differs"));
        }
        assert_origin_push_v1(
            uses,
            SourceOutputProjectionLiteralUseV1 {
                guard: segment.source_block,
                input: row.input,
                output,
            },
            budget,
        )
        .map_err(Error::SourceOrigin)?;
    }
    literal.end_use = uses.len();
    if literal.first_use == literal.end_use {
        return Err(Error::Invalid("literal has no retained checked guard use"));
    }
    Ok(SourceOutputProjectionArgumentV1 {
        ranked_value: claim.ranked_value,
        source_local: claim.source_local,
        component: claim.component,
        scalar,
        origin: SourceOutputProjectionLeafOriginV1::Literal(literal),
    })
}

fn source_output_control_literal_row_v1(
    row: SourceOutputControlUseRowV1,
    coordinate: fe2o3_kernel_ir::CanonicalKirUseCoordinateV1,
    value: ValueId,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputControlUseIdentityV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::{
        CanonicalKirDefinitionCoordinateV1 as Def, CanonicalKirUseCoordinateV1 as Use,
    };
    budget.charge_work(5).map_err(Error::Resource)?;
    if row.input.coordinate != coordinate
        || row.input.value != value
        || !matches!(coordinate, Use::OperationOperand { operand: 0, .. })
        || !matches!(row.input.definition, Def::Result { result: 0, .. })
    {
        return Err(Error::Invalid(
            "literal original Compare use or result slot differs",
        ));
    }
    let output = row
        .output
        .ok_or(Error::Invalid("literal Compare use was eliminated"))?;
    if !matches!(output.coordinate, Use::OperationOperand { operand: 0, .. })
        || !matches!(output.definition, Def::Result { result: 0, .. })
    {
        return Err(Error::Invalid(
            "literal output Compare use or result slot differs",
        ));
    }
    Ok(output)
}

fn source_output_control_ranked_anchor_v1(
    candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
    claim: ProductionProjectionArgumentCandidateV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<bool, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(2).map_err(Error::Resource)?;
    match claim.ranked_value {
        ProductionRankedValueV1::Argument(argument)
            if (argument as usize) < candidate.lowering.kernel().argument_count() =>
        {
            Ok(false)
        }
        ProductionRankedValueV1::Local(value) => {
            let mut found = None;
            for block in candidate.lowering.kernel().blocks() {
                budget.charge_work(1).map_err(Error::Resource)?;
                for operation in block.operations() {
                    budget.charge_work(2).map_err(Error::Resource)?;
                    let anchor = match operation {
                        ProductionRankedOperationV1::IndexUnknown { result }
                            if *result == value =>
                        {
                            Some(false)
                        }
                        ProductionRankedOperationV1::InvocationIndex {
                            result,
                            dimension: 0,
                            launch_extent: 0,
                        } if *result == value => Some(true),
                        _ => None,
                    };
                    if let Some(invocation) = anchor {
                        if found.replace(invocation).is_some() {
                            return Err(Error::Invalid("control unknown definition duplicated"));
                        }
                    }
                }
            }
            // The lowering owner already checked complete SSA definitions and
            // visibility. This check additionally requires the actual unknown
            // opcode, not a candidate assertion about its numerical meaning.
            if let Some(invocation) = found {
                Ok(invocation)
            } else {
                Err(Error::Invalid(
                    "control local anchor is not an actual IndexUnknown",
                ))
            }
        }
        _ => Err(Error::Invalid("control ranked leaf anchor unsupported")),
    }
}

// The wrapper recognizes checked SliceLength formals and exact invocation roots
// sealed by source/N/O use bindings. It does not evaluate Calls, loads or phis.
struct SourceOutputControlNormalizationV1<'a, 'kir, 'ranked, 'ledger, 'limit> {
    inner: SourceOutputScalarNormalizationV1<'a, 'kir, 'ranked, 'ledger, 'limit>,
    literal_uses: &'a [SourceOutputProjectionLiteralUseV1],
    guard: Option<(SemanticBlockIdV1, ValueId)>,
    invocation_roots: &'a [(fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1, u32)],
}

impl<'kir, 'ranked> ScalarNormalizationContextV1<'kir, 'ranked>
    for SourceOutputControlNormalizationV1<'_, 'kir, 'ranked, '_, '_>
{
    type Node = usize;
    fn charge(&mut self) -> Option<()> {
        self.inner.charge()
    }
    fn enter(&mut self, value: ValueId) -> Option<bool> {
        self.inner.enter(value)
    }
    fn leave(&mut self, value: ValueId) {
        self.inner.leave(value)
    }
    fn emit(&mut self, node: NormalizedScalarNodeV1<usize>) -> Option<usize> {
        self.inner.emit(node)
    }
    fn unique_origin(&mut self, value: ValueId) -> Option<ValueId> {
        self.inner.unique_origin(value)
    }
    fn parameter(
        &mut self,
        value: ValueId,
    ) -> Option<Option<(u32, ProductionSemanticScalarTypeV2)>> {
        use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
        let found = self
            .inner
            .inventory
            .definition_for_value(self.inner.function.coordinate, value, self.inner.budget)
            .map_err(ProductionSourceOutputErrorV1::Inventory);
        let definition = self.inner.remember(found)??;
        self.inner.paid(3)?;
        if let Definition::FunctionArgument { argument, .. } = definition.coordinate {
            return Some(Some((
                argument.checked_mul(2)?,
                kir_semantic_scalar_v1(definition.ty)?,
            )));
        }
        if !self.invocation_roots.is_empty() {
            let found =
                assert_origin_find_v1(self.invocation_roots, self.inner.budget, |row, budget| {
                    budget.charge_work(4)?;
                    Ok(row.0.cmp(&definition.coordinate))
                })
                .map_err(ProductionSourceOutputErrorV1::SourceOrigin);
            if let Some(index) = self.inner.remember(found)? {
                let slot = self.invocation_roots[index].1;
                self.inner.paid(2)?;
                PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2.checked_add(slot)?;
                return Some(Some((slot, source_output_address_u64_v1())));
            }
        }
        let Some(operation) = self.inner.operation(value) else {
            return Some(None);
        };
        let OperationKind::SliceLength { slice } = &operation.kind else {
            return Some(None);
        };
        self.inner.paid(operation.results.len().checked_add(3)?)?;
        if operation.results.len() != 1
            || operation.results[0].id != value
            || operation.results[0].ty != Type::INDEX
        {
            return None;
        }
        let slice = self.inner.unique_origin(*slice)?;
        let found = self
            .inner
            .inventory
            .definition_for_value(self.inner.function.coordinate, slice, self.inner.budget)
            .map_err(ProductionSourceOutputErrorV1::Inventory);
        let definition = self.inner.remember(found)??;
        let Definition::FunctionArgument { argument, .. } = definition.coordinate else {
            return None;
        };
        if !matches!(definition.ty, Type::Slice(_)) {
            return None;
        }
        Some(Some((
            argument.checked_mul(2)?.checked_add(1)?,
            ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 64,
            },
        )))
    }
    fn operation(&mut self, value: ValueId) -> Option<&'kir Operation> {
        self.inner.operation(value)
    }
    fn scalar(&mut self, value: ValueId) -> Option<ProductionSemanticScalarTypeV2> {
        self.inner.scalar(value)
    }
    fn load_site(&mut self, _: ValueId) -> Option<SemanticAccessSiteV1> {
        None
    }
    fn ranked_site(&mut self, _: (u32, u32)) -> Option<SemanticAccessSiteV1> {
        None
    }
    fn ranked_source(&mut self, _: SemanticAccessSiteV1) -> Option<IndexedRankedAccessSourceV1> {
        None
    }
    fn ranked_view(&mut self, _: ProductionRankedValueIdV1) -> Option<RankedViewDefinitionV1> {
        None
    }
    fn ranked_operation(&mut self, _: (u32, u32)) -> Option<&'ranked ProductionRankedOperationV1> {
        None
    }
}

fn source_output_control_ranked_leaf_v1(
    value: ProductionRankedValueV1,
    scalar: ProductionSemanticScalarTypeV2,
    arguments: &[SourceOutputProjectionArgumentV1],
    context: &mut SourceOutputControlNormalizationV1<'_, '_, '_, '_, '_>,
) -> Option<usize> {
    use NormalizedScalarNodeV1 as Node;
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
    context.inner.paid(3)?;
    let found = assert_origin_find_v1(arguments, context.inner.budget, |row, budget| {
        budget.charge_work(1)?;
        Ok(row.ranked_value.cmp(&value))
    })
    .map_err(ProductionSourceOutputErrorV1::SourceOrigin);
    if let Some(index) = context.inner.remember(found)? {
        let row = arguments.get(index)?;
        context.inner.paid(4)?;
        if row.scalar != scalar {
            return None;
        }
        let formal = match row.origin {
            SourceOutputProjectionLeafOriginV1::Formal(formal) => formal,
            SourceOutputProjectionLeafOriginV1::Invocation(invocation) => {
                let (guard, condition) = context.guard?;
                let uses = context
                    .literal_uses
                    .get(invocation.first_use..invocation.end_use)?;
                let mut found = None;
                for used in uses {
                    context.inner.paid(2)?;
                    if used.guard == guard && found.replace(*used).is_some() {
                        return None;
                    }
                }
                let used = found?;
                let definition = context
                    .inner
                    .inventory
                    .definition_for_value(
                        context.inner.function.coordinate,
                        condition,
                        context.inner.budget,
                    )
                    .map_err(ProductionSourceOutputErrorV1::Inventory);
                let definition = context.inner.remember(definition)??;
                context.inner.paid(3)?;
                let Definition::Result {
                    operation,
                    result: 0,
                } = definition.coordinate
                else {
                    return None;
                };
                if used.output.coordinate
                    != (fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
                        operation,
                        operand: 0,
                    })
                {
                    return None;
                }
                let symbol =
                    PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2.checked_add(invocation.symbol)?;
                let actual = normalize_kir_expression_core_v1(used.output.value, 0, context)?;
                if !matches!(context.inner.nodes.get(actual), Some(Node::Symbol { scalar: ty, symbol: id })
                    if *ty == scalar && *id == symbol)
                {
                    return None;
                }
                return context.emit(Node::Symbol { scalar, symbol });
            }
            SourceOutputProjectionLeafOriginV1::Literal(literal) => {
                let (guard, condition) = context.guard?;
                let uses = context
                    .literal_uses
                    .get(literal.first_use..literal.end_use)?;
                let mut found = None;
                for used in uses {
                    context.inner.paid(2)?;
                    if used.guard == guard && found.replace(*used).is_some() {
                        return None;
                    }
                }
                let used = found?;
                let definition = context
                    .inner
                    .inventory
                    .definition_for_value(
                        context.inner.function.coordinate,
                        condition,
                        context.inner.budget,
                    )
                    .map_err(ProductionSourceOutputErrorV1::Inventory);
                let definition = context.inner.remember(definition)??;
                context.inner.paid(3)?;
                let Definition::Result {
                    operation,
                    result: 0,
                } = definition.coordinate
                else {
                    return None;
                };
                if used.output.coordinate
                    != (fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
                        operation,
                        operand: 0,
                    })
                {
                    return None;
                }
                let actual = normalize_kir_expression_core_v1(used.output.value, 0, context)?;
                if !matches!(context.inner.nodes.get(actual), Some(Node::Constant { scalar: ty, bits })
                    if *ty == scalar && *bits == literal.bits)
                {
                    return None;
                }
                return context.emit(Node::Constant {
                    scalar,
                    bits: literal.bits,
                });
            }
        };
        let Definition::FunctionArgument { argument, .. } = formal.output else {
            return None;
        };
        let slot = argument.checked_mul(2)?.checked_add(u32::from(
            row.component == ProductionProjectionArgumentComponentV1::SliceLength,
        ))?;
        return context.emit(Node::Symbol {
            symbol: PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2.checked_add(slot)?,
            scalar,
        });
    }
    match value {
        ProductionRankedValueV1::Local(value) => {
            // A paid search is bounded by the caller ledger. The initial grammar
            // accepts literal leaves only, never opaque generated index values.
            let mut literal = None;
            for block in context.inner.lowering.kernel().blocks() {
                context.inner.paid(1)?;
                for operation in block.operations() {
                    context.inner.paid(2)?;
                    if let ProductionRankedOperationV1::IndexConstant {
                        result,
                        value: bits,
                    } = operation
                        && *result == value
                        && literal.replace(*bits).is_some()
                    {
                        return None;
                    }
                }
            }
            let bits = literal?;
            let width = source_output_control_unsigned_width_v1(scalar)?;
            if width < 64 && bits >= (1u64 << width) {
                return None;
            }
            context.emit(Node::Constant { scalar, bits })
        }
        ProductionRankedValueV1::Argument(_) | ProductionRankedValueV1::BlockArgument { .. } => {
            None
        }
    }
}

fn source_output_control_unsigned_width_v1(scalar: ProductionSemanticScalarTypeV2) -> Option<u32> {
    use ProductionSemanticScalarTypeV2 as Scalar;
    match scalar {
        Scalar::Integer {
            signed: false,
            bits: bits @ (8 | 16 | 32 | 64),
        } => Some(u32::from(bits)),
        Scalar::Bool => Some(1),
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct SourceOutputProjectionSegmentV1 {
    source: SemanticBlockIdV1,
    original: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
    placement: fe2o3_kernel_analysis::CanonicalKirBlockPlacementV1,
    claim: ProductionProjectionControlBlockV1,
}

fn source_output_control_edge_v1<'a>(
    rows: &'a SourceOutputCheckedControlRowsV1,
    coordinate: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<&'a SourceOutputControlEdgeRowV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let index = assert_origin_find_v1(&rows.edges, budget, |row, budget| {
        budget.charge_work(3)?;
        Ok(row.input.coordinate.cmp(&coordinate))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid("control input edge absent"))?;
    Ok(&rows.edges[index])
}

fn source_output_control_argument_rows_v1(
    rows: &SourceOutputCheckedControlRowsV1,
    edge: &SourceOutputControlEdgeRowV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    for argument in 0..edge.input.argument_count {
        budget.charge_work(1).map_err(Error::Resource)?;
        let coordinate = fe2o3_kernel_ir::CanonicalKirEdgeArgumentCoordinateV1 {
            edge: edge.input.coordinate,
            argument,
        };
        let index = assert_origin_find_v1(&rows.arguments, budget, |row, budget| {
            budget.charge_work(4)?;
            Ok(row.input.coordinate.cmp(&coordinate))
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("control edge argument absent"))?;
        budget.charge_work(3).map_err(Error::Resource)?;
        if let Some(output) = &edge.output {
            let actual = rows.arguments[index]
                .output
                .ok_or(Error::Invalid("control retained edge argument omitted"))?;
            if actual.coordinate.edge != output.coordinate || actual.coordinate.argument != argument
            {
                return Err(Error::Invalid("control edge argument occurrence differs"));
            }
        }
    }
    Ok(())
}

fn source_output_control_trap_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
    edge: &SourceOutputControlEdgeRowV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1 as Placement;
    budget.charge_work(6).map_err(Error::Resource)?;
    let output = match edge.checked.placement {
        Placement::Retained(_) => {
            edge.output
                .as_ref()
                .ok_or(Error::Invalid("control retained edge output absent"))?
                .target
        }
        Placement::InternalConnector(placement) => placement.output,
        Placement::Omitted => return Err(Error::Invalid("live trap edge omitted")),
    };
    let block = view
        .output()
        .module()
        .functions
        .get(output.function.0 as usize)
        .and_then(|function| function.body.as_ref())
        .and_then(|body| body.blocks.get(output.block as usize))
        .ok_or(Error::Invalid("control output trap block absent"))?;
    let operation = block
        .operations
        .len()
        .checked_sub(1)
        .ok_or(Error::Invalid("control output trap operation absent"))?;
    let operation = u32::try_from(operation)
        .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    let before = view
        .source
        .executable()
        .module()
        .functions
        .get(edge.input.target.function.0 as usize)
        .and_then(|function| function.body.as_ref())
        .and_then(|body| body.blocks.get(edge.input.target.block as usize))
        .ok_or(Error::Invalid("control original trap target absent"))?;
    source_output_census_trap_span_v1(
        view.source.correspondence.synthetic_operation_spans(),
        candidate.selected_root,
        candidate.selected_function,
        before.id,
        0,
        budget,
    )?;
    source_output_census_trap_payload_v1(
        before,
        block,
        operation as usize,
        (
            fe2o3_kernel_ir::FunctionRole::KernelEntry,
            fe2o3_kernel_ir::FunctionRole::KernelEntry,
        ),
        budget,
    )?;
    view.census_runtime_failure_trap_v1(
        candidate.selected_root,
        candidate.selected_function,
        fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
            block: output,
            operation,
        },
        budget,
    )
}

fn source_output_control_target_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
    segments: &[SourceOutputProjectionSegmentV1],
    edge: &SourceOutputControlEdgeRowV1,
    ranked_target: u32,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let index = assert_origin_find_v1(segments, budget, |row, budget| {
        budget.charge_work(2)?;
        Ok(row.original.cmp(&edge.input.target))
    })
    .map_err(Error::SourceOrigin)?;
    budget.charge_work(3).map_err(Error::Resource)?;
    if let Some(index) = index {
        if ranked_target != segments[index].claim.first {
            return Err(Error::Invalid("control projected edge target differs"));
        }
    } else {
        source_output_control_trap_v1(view, candidate, edge, budget)?;
        let target = candidate
            .lowering
            .kernel()
            .blocks()
            .get(ranked_target as usize)
            .ok_or(Error::Invalid("control projected trap target absent"))?;
        if !target.operations().is_empty()
            || !matches!(target.terminator(), ProductionRankedTerminatorV1::Trap)
        {
            return Err(Error::Invalid(
                "control actual failure was changed to a continuation",
            ));
        }
    }
    Ok(())
}

fn source_output_control_predicate_v1(
    condition: ValueId,
    ranked: &ProductionRankedTerminatorV1,
    arguments: &[SourceOutputProjectionArgumentV1],
    context: &mut SourceOutputControlNormalizationV1<'_, '_, '_, '_, '_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use NormalizedScalarNodeV1 as Node;
    use ProductionSourceOutputErrorV1 as Error;
    context
        .inner
        .paid(3)
        .ok_or_else(|| context.inner.failure())?;
    context.inner.nodes.clear();
    context.inner.normalization_steps = 0;
    let actual = normalize_kir_expression_core_v1(condition, 0, context)
        .ok_or_else(|| context.inner.failure())?;
    let (operation, lhs, rhs) = match ranked {
        ProductionRankedTerminatorV1::IndexLessThan { lhs, rhs, .. } => {
            (ProductionSemanticComparisonV2::LessThan, *lhs, *rhs)
        }
        ProductionRankedTerminatorV1::IndexEqual { lhs, rhs, .. } => {
            (ProductionSemanticComparisonV2::Equal, *lhs, *rhs)
        }
        _ => {
            return Err(Error::Invalid(
                "control projected predicate grammar unsupported",
            ));
        }
    };
    if matches!(
        context.inner.nodes.get(actual),
        Some(Node::Symbol {
            scalar: ProductionSemanticScalarTypeV2::Bool,
            ..
        })
    ) {
        if operation != ProductionSemanticComparisonV2::Equal {
            return Err(Error::Invalid(
                "control Boolean predicate comparison differs",
            ));
        }
        let left = source_output_control_ranked_leaf_v1(
            lhs,
            ProductionSemanticScalarTypeV2::Bool,
            arguments,
            context,
        )
        .ok_or_else(|| context.inner.failure())?;
        let right = source_output_control_ranked_leaf_v1(
            rhs,
            ProductionSemanticScalarTypeV2::Bool,
            arguments,
            context,
        )
        .ok_or_else(|| context.inner.failure())?;
        if !matches!(
            context.inner.nodes.get(right),
            Some(Node::Constant {
                scalar: ProductionSemanticScalarTypeV2::Bool,
                bits: 1
            })
        ) || !context
            .inner
            .equivalent(actual, left)
            .ok_or_else(|| context.inner.failure())?
        {
            return Err(Error::Invalid("control Boolean source argument differs"));
        }
        return Ok(());
    }
    let (expected_operation, scalar) = match context.inner.nodes.get(actual) {
        Some(Node::Compare {
            operation,
            operand_scalar,
            ..
        }) => (*operation, *operand_scalar),
        _ => {
            return Err(Error::Invalid(
                "control output predicate is not a supported comparison",
            ));
        }
    };
    if operation != expected_operation || source_output_control_unsigned_width_v1(scalar).is_none()
    {
        return Err(Error::Invalid("control projected comparison differs"));
    }
    let lhs = source_output_control_ranked_leaf_v1(lhs, scalar, arguments, context)
        .ok_or_else(|| context.inner.failure())?;
    let rhs = source_output_control_ranked_leaf_v1(rhs, scalar, arguments, context)
        .ok_or_else(|| context.inner.failure())?;
    let expected = context
        .emit(Node::Compare {
            operation,
            operand_scalar: scalar,
            lhs,
            rhs,
        })
        .ok_or_else(|| context.inner.failure())?;
    if !context
        .inner
        .equivalent(actual, expected)
        .ok_or_else(|| context.inner.failure())?
    {
        return Err(Error::Invalid(
            "control projected predicate operands differ",
        ));
    }
    Ok(())
}

fn source_output_control_segments_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
    identity: Option<&SourceOutputIdentityGetterV1>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<Vec<SourceOutputProjectionSegmentV1>, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(4).map_err(Error::Resource)?;
    budget
        .reserve_storage(std::mem::size_of::<Vec<SourceOutputProjectionSegmentV1>>())
        .map_err(Error::Resource)?;
    let mut segments = Vec::new();
    let function = view
        .source
        .semantic_ssa()
        .source_semantic()
        .functions()
        .get(candidate.selected_function.index() as usize)
        .ok_or(Error::Invalid("control source function absent"))?;
    let blocks = candidate.lowering.kernel().blocks();
    let mut next = 1u32;
    let mut claimed = 0usize;
    for (ordinal, block) in function.blocks().iter().enumerate() {
        budget.charge_work(4).map_err(Error::Resource)?;
        let source = SemanticBlockIdV1::from_index(
            u32::try_from(ordinal)
                .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
        );
        let disposition = view.block(
            candidate.selected_root,
            candidate.selected_function,
            source,
            budget,
        )?;
        if identity.is_some_and(|identity| identity.source.fallback == Some(source)) {
            budget.charge_work(2).map_err(Error::Resource)?;
            if candidate
                .control
                .blocks
                .get(claimed)
                .is_some_and(|row| row.source_block == source)
            {
                return Err(Error::Invalid(
                    "identity impossible default acquired a projected segment",
                ));
            }
            // The independent identity edge check below validates the complete
            // typed default occurrence and its actual N/O empty Unreachable.
            continue;
        }
        if matches!(
            disposition,
            ProductionSourceOutputBlockV1::Materialized {
                executable: true,
                placement: None,
                ..
            }
        ) {
            return Err(Error::Invalid(
                "control executable source lacks output placement",
            ));
        }
        let ProductionSourceOutputBlockV1::Materialized {
            original,
            placement: Some(placement),
            executable: true,
        } = disposition
        else {
            // Omission and retained nonexecution are still examined, not a
            // license to erase a live assertion based on its access target.
            if candidate
                .control
                .blocks
                .get(claimed)
                .is_some_and(|row| row.source_block == source)
            {
                return Err(Error::Invalid(
                    "control dormant source has a projected segment",
                ));
            }
            continue;
        };
        match block.terminator().kind() {
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::SwitchInt { .. }
            | SemanticTerminatorKindV1::Assert { .. }
            | SemanticTerminatorKindV1::Return => {}
            SemanticTerminatorKindV1::Call(call) => {
                if call.destination().is_none()
                    || matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
                {
                    return Err(Error::Invalid(
                        "control ordinary Call continuation or unwind unsupported",
                    ));
                }
            }
            _ => {
                return Err(Error::Invalid(
                    "control source terminal grammar unsupported",
                ));
            }
        }
        let claim = *candidate
            .control
            .blocks
            .get(claimed)
            .ok_or(Error::Invalid("control live source segment absent"))?;
        if claim.source_block != source
            || claim.first != next
            || claim.tail != claim.first
            || !(claim.end
                == claim
                    .tail
                    .checked_add(1)
                    .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?
                || claim.end
                    == claim
                        .tail
                        .checked_add(2)
                        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?)
            || claim.end as usize > blocks.len()
        {
            return Err(Error::Invalid(
                "control segment partition or internal expansion unsupported",
            ));
        }
        for block in &blocks[claim.first as usize..claim.end as usize] {
            budget.charge_work(2).map_err(Error::Resource)?;
            if block.index_argument_count() != 0 {
                return Err(Error::Invalid(
                    "control generated index-argument transport unsupported",
                ));
            }
        }
        if claim.end == claim.tail + 2 {
            let trap = &blocks[claim.tail as usize + 1];
            if !trap.operations().is_empty()
                || !matches!(trap.terminator(), ProductionRankedTerminatorV1::Trap)
            {
                return Err(Error::Invalid(
                    "control generated failure block is not an empty trap",
                ));
            }
        }
        assert_origin_push_v1(
            &mut segments,
            SourceOutputProjectionSegmentV1 {
                source,
                original,
                placement,
                claim,
            },
            budget,
        )
        .map_err(Error::SourceOrigin)?;
        next = claim.end;
        claimed += 1;
    }
    budget.charge_work(4).map_err(Error::Resource)?;
    if claimed != candidate.control.blocks.len() || next as usize != blocks.len() {
        return Err(Error::Invalid(
            "control projection has extra or missing segments",
        ));
    }
    // Pay the complete small lookup, even if the matching entry was first.
    budget
        .charge_work(segments.len())
        .map_err(Error::Resource)?;
    let entry = segments
        .iter()
        .find(|segment| segment.source == function.entry())
        .ok_or(Error::Invalid("control source entry segment absent"))?;
    let prelude = blocks
        .first()
        .ok_or(Error::Invalid("control projected entry absent"))?;
    if prelude.index_argument_count() != 0
        || !matches!(prelude.terminator(), ProductionRankedTerminatorV1::Branch { target } if *target == entry.claim.first)
    {
        return Err(Error::Invalid("control projected entry edge differs"));
    }
    source_output_ranked_sort_unique_v1(&mut segments, budget, |a, b| a.original.cmp(&b.original))?;
    Ok(segments)
}

fn source_output_control_calls_and_accesses_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
    canonical: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    segments: &[SourceOutputProjectionSegmentV1],
    arguments: &[SourceOutputProjectionArgumentV1],
    has_invocation: bool,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(4).map_err(Error::Resource)?;
    if !candidate.executable_effect_sources.is_empty() {
        return Err(Error::Invalid(
            "conditional control generated effects are unsupported",
        ));
    }
    let source = view
        .source
        .semantic_ssa()
        .source_semantic()
        .functions()
        .get(candidate.selected_function.index() as usize)
        .ok_or(Error::Invalid("control call source function absent"))?;
    budget
        .reserve_storage(
            std::mem::size_of::<Vec<u32>>()
                + std::mem::size_of::<Vec<&ProductionRankedAccessSourceV1>>(),
        )
        .map_err(Error::Resource)?;
    let mut call_counts = Vec::new();
    for _ in source.blocks() {
        assert_origin_push_v1(&mut call_counts, 0u32, budget).map_err(Error::SourceOrigin)?;
    }
    let output = view.source_output_exact_entry_v1(
        candidate.selected_root,
        candidate.selected_function,
        budget,
    )?;
    let body = output
        .body
        .as_ref()
        .ok_or(Error::Invalid("control call output body absent"))?;
    for (block_index, block) in body.blocks.iter().enumerate() {
        budget.charge_work(1).map_err(Error::Resource)?;
        for (operation_index, operation) in block.operations.iter().enumerate() {
            budget.charge_work(2).map_err(Error::Resource)?;
            if !matches!(operation.kind, OperationKind::Call { .. }) {
                continue;
            }
            let coordinate = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                    function: canonical,
                    block: u32::try_from(block_index)
                        .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                },
                operation: u32::try_from(operation_index)
                    .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
            };
            let Some(bindings) = view.retained_ordinary_call_bindings_v1(
                candidate.selected_root,
                candidate.selected_function,
                coordinate,
                budget,
            )?
            else {
                view.census_runtime_failure_trap_v1(
                    candidate.selected_root,
                    candidate.selected_function,
                    coordinate,
                    budget,
                )?;
                continue;
            };
            // The cached closure checked every transitive CFG, typed argument,
            // result and Return. These slots remain actual O, not evaluated or
            // copied into the ranked graph. Zero Returns is deliberately legal.
            let slots = bindings
                .arguments
                .len()
                .checked_add(bindings.results.len())
                .and_then(|n| n.checked_add(bindings.returns.len()))
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            budget
                .charge_work(
                    slots
                        .checked_add(5)
                        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                )
                .map_err(Error::Resource)?;
            let source_block = bindings.diagnostic.source_block;
            if !matches!(
                view.block(
                    candidate.selected_root,
                    candidate.selected_function,
                    source_block,
                    budget
                )?,
                ProductionSourceOutputBlockV1::Materialized {
                    executable: true,
                    ..
                }
            ) {
                continue;
            }
            let source_call = source
                .blocks()
                .get(source_block.index() as usize)
                .ok_or(Error::Invalid("control retained Call source block absent"))?;
            let SemanticTerminatorKindV1::Call(call) = source_call.terminator().kind() else {
                return Err(Error::Invalid(
                    "control actual Call has no source Call boundary",
                ));
            };
            if call.destination().is_none() {
                return Err(Error::Invalid(
                    "control nonreturning source Call has no conditional continuation",
                ));
            }
            budget
                .charge_work(segments.len())
                .map_err(Error::Resource)?;
            let segment = segments
                .iter()
                .find(|row| row.source == source_block)
                .ok_or(Error::Invalid(
                    "control retained Call has no live projected boundary",
                ))?;
            if segment.placement.output != coordinate.block {
                return Err(Error::Invalid(
                    "control actual Call escaped its checked source segment",
                ));
            }
            let count = call_counts
                .get_mut(source_block.index() as usize)
                .ok_or(Error::Invalid("control Call count source absent"))?;
            if *count != 0 {
                return Err(Error::Invalid("control source Call boundary duplicated"));
            }
            *count = 1;
        }
    }
    for segment in segments {
        budget.charge_work(3).map_err(Error::Resource)?;
        let SemanticTerminatorKindV1::Call(call) = source.blocks()[segment.source.index() as usize]
            .terminator()
            .kind()
        else {
            continue;
        };
        let mut intrinsic = false;
        if has_invocation {
            for argument in arguments {
                budget.charge_work(1).map_err(Error::Resource)?;
                if let SourceOutputProjectionLeafOriginV1::Invocation(anchor) = argument.origin {
                    budget.charge_work(4).map_err(Error::Resource)?;
                    intrinsic |= anchor.source.get.source().get() == segment.source.index()
                        || anchor.source.producer.source().get() == segment.source.index();
                }
            }
        }
        if intrinsic {
            budget
                .charge_work(
                    segments
                        .len()
                        .checked_add(8)
                        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                )
                .map_err(Error::Resource)?;
            if call_counts[segment.source.index() as usize] != 0 {
                return Err(Error::Invalid(
                    "invocation boundary retains an ordinary Call",
                ));
            }
            let destination = call
                .destination()
                .ok_or(Error::Invalid("invocation continuation absent"))?;
            let target = segments
                .iter()
                .find(|row| row.source == destination.edge().target())
                .ok_or(Error::Invalid("invocation live successor segment absent"))?;
            let edge = source_output_control_edge_v1(
                &view.checked_control_rows,
                fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1 {
                    source: segment.original,
                    successor: 0,
                },
                budget,
            )?;
            if edge.input.target != target.original
                || !edge.checked.executable
                || matches!(
                    edge.checked.placement,
                    fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1::Omitted
                )
            {
                return Err(Error::Invalid("invocation checked continuation differs"));
            }
            source_output_control_argument_rows_v1(&view.checked_control_rows, edge, budget)?;
            // The common segment checker below authenticates the retained edge
            // or InternalConnector placement. No synthetic O branch is required.
        } else if call_counts[segment.source.index() as usize] != 1 {
            return Err(Error::Invalid(
                "control projected continuation lost its actual Call",
            ));
        }
    }
    let mut accesses = Vec::new();
    for source in candidate.access_sources {
        assert_origin_push_v1(&mut accesses, source, budget).map_err(Error::SourceOrigin)?;
    }
    source_output_ranked_sort_unique_v1(&mut accesses, budget, |a, b| {
        (
            a.semantic_block,
            a.semantic_statement,
            a.semantic_access_ordinal,
        )
            .cmp(&(
                b.semantic_block,
                b.semantic_statement,
                b.semantic_access_ordinal,
            ))
    })?;
    let mut previous = None;
    for access in accesses {
        budget.charge_work(6).map_err(Error::Resource)?;
        if access.semantic_statement.is_none() {
            return Err(Error::Invalid(
                "control terminator memory expansion unsupported",
            ));
        }
        budget
            .charge_work(segments.len())
            .map_err(Error::Resource)?;
        let segment = segments
            .iter()
            .find(|row| row.source.index() == access.semantic_block)
            .ok_or(Error::Invalid("control access has no live source segment"))?;
        if access.ranked_block != segment.claim.first {
            return Err(Error::Invalid(
                "control access moved across a source boundary",
            ));
        }
        let location = (access.ranked_block, access.ranked_operation);
        if let Some((block, last)) = previous
            && block == access.semantic_block
            && last >= location
        {
            return Err(Error::Invalid("control source effect order differs"));
        }
        previous = Some((access.semantic_block, location));
    }
    Ok(())
}

impl ProductionSourceOutputOccurrencesV1<'_, '_> {
    /// Complete a source-root-ordered batch on the caller's canonical ledger.
    /// Temporary indexes drop before floor restoration on Result and unwind;
    /// this implementation never resets accepted work or the first denial.
    /// Callers must preserve the original Budget and live reservations; address
    /// and floor checks do not detect arbitrary in-place ledger replacement.
    /// The callback cannot
    /// export the borrowed capability. It must nest the complete effect/Store
    /// census before claiming a completed memory-analysis result.
    pub fn with_conditional_memory_control_coverage_v1<T>(
        &self,
        candidates: &[ProductionCanonicalMemoryAnalysisCandidateV1<'_>],
        budget: &mut AssertOriginBudgetV1<'_>,
        body: impl for<'scope> FnOnce(
            &ProductionConditionalMemoryControlCoverageV1<'scope>,
            &mut AssertOriginBudgetV1<'_>,
        ) -> Result<T, ProductionSourceOutputErrorV1>,
    ) -> Result<T, ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        budget.charge_work(6).map_err(Error::Resource)?;
        let minimum = self
            .source
            .executable_storage()
            .retained_storage()
            .checked_add(self.source.assert_origin_storage().payload_storage())
            .and_then(|n| n.checked_add(self.checked_output.storage().retained_storage()))
            .and_then(|n| n.checked_add(self.storage.retained_storage()))
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        if budget.storage() < minimum {
            return Err(Error::Resource(AssertOriginResourceV1::Accounting));
        }
        if candidates.len() != self.source.launch_roots.len() {
            return Err(Error::Invalid("conditional control root roster differs"));
        }
        source_output_global_scratch_scope_v1(budget, |budget| {
            budget.charge_work(1).map_err(Error::Resource)?;
            let header = std::mem::size_of::<Vec<SourceOutputControlCandidateIdentityV1<'_>>>()
                + std::mem::size_of::<Vec<SourceOutputProjectionArgumentV1>>()
                + std::mem::size_of::<Vec<SourceOutputProjectionLiteralUseV1>>()
                + std::mem::size_of::<Vec<SourceOutputInvocationSourceIndexV1>>()
                + std::mem::size_of::<
                    Vec<(fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1, u32)>,
                >()
                + std::mem::size_of::<ProductionConditionalMemoryControlCoverageV1<'_>>();
            budget.reserve_storage(header).map_err(Error::Resource)?;
            let mut identities = Vec::new();
            let mut arguments: Vec<SourceOutputProjectionArgumentV1> = Vec::new();
            let mut literal_uses = Vec::new();
            let mut invocation_sources = Vec::new();
            let mut invocation_roots = Vec::new();
            let (inventory, storage) =
                fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(self.output(), budget)
                    .map_err(Error::Inventory)?;
            budget
                .reserve_storage(storage.retained_storage())
                .map_err(Error::Resource)?;
            for (candidate, root) in candidates.iter().zip(self.source.launch_roots.iter()) {
                budget.charge_work(3).map_err(Error::Resource)?;
                if candidate.selected_root != root.selected_root {
                    return Err(Error::Invalid("conditional control ordered root differs"));
                }
                let canonical = source_output_ordinary_function_alias_v1(
                    self.source,
                    candidate.selected_root,
                    candidate.selected_function,
                    budget,
                )?;
                self.source_output_exact_entry_v1(
                    candidate.selected_root,
                    candidate.selected_function,
                    budget,
                )?;
                let first = arguments.len();
                let mut invocation_index = None;
                let mut identity = None;
                for claim in &candidate.control.arguments {
                    budget.charge_work(2).map_err(Error::Resource)?;
                    let invocation =
                        source_output_control_ranked_anchor_v1(candidate, *claim, budget)?;
                    let row = if invocation {
                        let ordinal = if let Some(ordinal) = invocation_index {
                            ordinal
                        } else {
                            let index = source_output_invocation_source_index_v1(
                                self, candidate, canonical, budget,
                            )?;
                            let ordinal = invocation_sources.len();
                            assert_origin_push_v1(&mut invocation_sources, index, budget)
                                .map_err(Error::SourceOrigin)?;
                            // The moved index is now charged in the vector's capacity.
                            budget
                                .release_storage(std::mem::size_of::<
                                    SourceOutputInvocationSourceIndexV1,
                                >())
                                .map_err(Error::Resource)?;
                            invocation_index = Some(ordinal);
                            ordinal
                        };
                        // The existing invocation entry charge covers component
                        // validation and this fixed-size type dispatch.
                        budget.charge_work(4).map_err(Error::Resource)?;
                        if claim.component != ProductionProjectionArgumentComponentV1::Scalar {
                            return Err(Error::Invalid(
                                "invocation source component is not scalar",
                            ));
                        }
                        let source = self.source.semantic_ssa().source_semantic();
                        let aggregate_source = matches!(
                            source.functions()[candidate.selected_function.index() as usize]
                                .locals()
                                .get(claim.source_local.index() as usize)
                                .and_then(|local| source.types().get(local.ty().index() as usize))
                                .map(SemanticTypeDeclV1::shape),
                            Some(SemanticTypeShapeV1::Aggregate(_))
                        );
                        let row = if !aggregate_source {
                            if identity.is_some() {
                                return Err(Error::Invalid(
                                    "identity getter cannot mix raw invocation anchors",
                                ));
                            }
                            source_output_control_invocation_v1(
                                self,
                                candidate,
                                &invocation_sources[ordinal],
                                ordinal,
                                *claim,
                                &mut literal_uses,
                                &inventory,
                                budget,
                            )?
                        } else {
                            budget
                                .charge_work(arguments.len() - first)
                                .map_err(Error::Resource)?;
                            if identity.is_some()
                                || arguments[first..].iter().any(|row| {
                                    matches!(
                                        row.origin,
                                        SourceOutputProjectionLeafOriginV1::Invocation(_)
                                    )
                                })
                            {
                                return Err(Error::Invalid(
                                    "identity getter invocation anchor is not unique",
                                ));
                            }
                            budget
                                .reserve_storage(std::mem::size_of::<
                                    Option<SourceOutputIdentityGetterV1>,
                                >())
                                .map_err(Error::Resource)?;
                            let (row, checked) = source_output_identity_getter_v1(
                                self,
                                candidate,
                                &invocation_sources[ordinal],
                                ordinal,
                                *claim,
                                &mut literal_uses,
                                &inventory,
                                budget,
                            )?;
                            identity = Some(checked);
                            row
                        };
                        if let SourceOutputProjectionLeafOriginV1::Invocation(anchor) = row.origin {
                            assert_origin_push_v1(
                                &mut invocation_roots,
                                (anchor.output, anchor.symbol),
                                budget,
                            )
                            .map_err(Error::SourceOrigin)?;
                        }
                        row
                    } else {
                        source_output_control_argument_v1(
                            self,
                            candidate,
                            canonical,
                            *claim,
                            &mut literal_uses,
                            &inventory,
                            budget,
                        )?
                    };
                    assert_origin_push_v1(&mut arguments, row, budget)
                        .map_err(Error::SourceOrigin)?;
                }
                source_output_ranked_sort_unique_v1(&mut arguments[first..], budget, |a, b| {
                    a.ranked_value.cmp(&b.ranked_value)
                })?;
                if invocation_index.is_some() {
                    source_output_invocation_sort_v1(
                        &mut invocation_roots,
                        4,
                        true,
                        |a, b| a.0.cmp(&b.0),
                        budget,
                    )?;
                }
                // Distinct ranked variables may not pretend to be independent
                // aliases of one source component. Global alpha-renaming is
                // harmless; substituting another source local is not.
                for (index, row) in arguments[first..].iter().enumerate() {
                    budget.charge_work(index).map_err(Error::Resource)?;
                    if arguments[first..first + index].iter().any(|other| {
                        (other.original(), other.component) == (row.original(), row.component)
                    }) {
                        return Err(Error::Invalid(
                            "control source argument component is duplicated",
                        ));
                    }
                }
                source_output_global_scratch_scope_v1(budget, |budget| {
                    let segments = source_output_control_segments_v1(
                        self,
                        candidate,
                        identity.as_ref(),
                        budget,
                    )?;
                    source_output_control_calls_and_accesses_v1(
                        self,
                        candidate,
                        canonical,
                        &segments,
                        &arguments[first..],
                        invocation_index.is_some(),
                        budget,
                    )?;
                    budget.charge_work(2).map_err(Error::Resource)?;
                    let function = inventory
                        .functions()
                        .get(canonical.0 as usize)
                        .ok_or(Error::Invalid("control output inventory function absent"))?;
                    let header = std::mem::size_of::<
                        SourceOutputControlNormalizationV1<'_, '_, '_, '_, '_>,
                    >()
                    .checked_sub(std::mem::size_of::<SourceOutputAllocationScratchV1>())
                    .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
                    budget.reserve_storage(header).map_err(Error::Resource)?;
                    let ssa = source_output_allocation_scratch_v1(&inventory, budget)?;
                    let mut context = SourceOutputControlNormalizationV1 {
                        literal_uses: &literal_uses,
                        guard: None,
                        invocation_roots: &invocation_roots,
                        inner: SourceOutputScalarNormalizationV1 {
                            inventory: &inventory,
                            function,
                            lowering: candidate.lowering,
                            sources: Vec::new(),
                            locations: Vec::new(),
                            sites: Vec::new(),
                            views: Vec::new(),
                            expressions: Vec::new(),
                            nodes: Vec::new(),
                            comparison: [(0, 0);
                                2 * MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
                            visiting: [None; MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
                            visiting_len: 0,
                            normalization_steps: 0,
                            ssa,
                            error: None,
                            budget,
                        },
                    };
                    for segment in &segments {
                        if let Some(identity) = &identity
                            && segment.source == identity.source.switch
                        {
                            source_output_identity_edges_v1(
                                self,
                                candidate,
                                &segments,
                                *segment,
                                identity,
                                &invocation_sources[invocation_index
                                    .ok_or(Error::Invalid("identity source index absent"))?],
                                &inventory,
                                context.inner.budget,
                            )?;
                            continue;
                        }
                        source_output_control_segment_edges_v1(
                            self,
                            candidate,
                            &segments,
                            *segment,
                            &arguments[first..],
                            &mut context,
                        )?;
                    }
                    if let Some(identity) = &mut identity {
                        source_output_identity_stores_v1(
                            self,
                            candidate,
                            identity,
                            &inventory,
                            context.inner.budget,
                        )?;
                    }
                    Ok(())
                })?;
                let identity_address = identity
                    .map(|identity| source_output_identity_address_move_v1(identity, budget))
                    .transpose()?;
                let moved_header = if identity_address.is_some() {
                    std::mem::size_of::<Option<SourceOutputIdentityAddressV1>>()
                } else {
                    0
                };
                assert_origin_push_v1(
                    &mut identities,
                    SourceOutputControlCandidateIdentityV1 {
                        candidate_identity: std::ptr::from_ref(candidate).cast::<()>(),
                        root: candidate.selected_root,
                        function: candidate.selected_function,
                        canonical,
                        lowering: candidate.lowering,
                        sources: candidate.access_sources,
                        effects: candidate.executable_effect_sources,
                        claims: candidate.control,
                        arguments: first..arguments.len(),
                        identity_address,
                    },
                    budget,
                )
                .map_err(Error::SourceOrigin)?;
                if moved_header != 0 {
                    // The moved summary header now occupies the paid candidate
                    // slot; its Store capacity stays reserved through the scope.
                    budget
                        .release_storage(moved_header)
                        .map_err(Error::Resource)?;
                }
            }
            let coverage = ProductionConditionalMemoryControlCoverageV1 {
                view_identity: std::ptr::from_ref(self).cast::<()>(),
                budget_identity: std::ptr::from_ref(budget).cast::<()>(),
                live_floor: budget.storage(),
                output: self.output(),
                inventory: &inventory,
                candidates: &identities,
                arguments: &arguments,
                literal_uses: &literal_uses,
                invocation_sources: &invocation_sources,
                invocation_roots: &invocation_roots,
            };
            body(&coverage, budget)
        })
    }
}

include!("production_semantic_kir_v1/tests/production_source_output_control_coverage_v1_tests.rs");

fn source_output_control_return_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    segment: SourceOutputProjectionSegmentV1,
    values: &[ValueId],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::CanonicalKirUseCoordinateV1 as Use;
    budget.charge_work(5).map_err(Error::Resource)?;
    let output = view
        .output()
        .module()
        .functions
        .get(segment.placement.output.function.0 as usize)
        .and_then(|function| function.body.as_ref())
        .and_then(|body| body.blocks.get(segment.placement.output.block as usize))
        .and_then(|block| block.terminator.as_ref())
        .ok_or(Error::Invalid("control actual output Return absent"))?;
    let Terminator::Return { values: output } = output else {
        return Err(Error::Invalid(
            "control actual output terminal is not Return",
        ));
    };
    if output.len() != values.len() {
        return Err(Error::Invalid("control actual Return arity differs"));
    }
    for (operand, (input_value, output_value)) in values.iter().zip(output).enumerate() {
        budget.charge_work(3).map_err(Error::Resource)?;
        let operand = u32::try_from(operand)
            .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        let coordinate = Use::TerminatorOperand {
            block: segment.original,
            operand,
        };
        let ordinal =
            assert_origin_find_v1(&view.checked_control_rows.uses, budget, |row, budget| {
                budget.charge_work(3)?;
                Ok(row.input.coordinate.cmp(&coordinate))
            })
            .map_err(Error::SourceOrigin)?
            .ok_or(Error::Invalid("control checked Return use absent"))?;
        budget.charge_work(5).map_err(Error::Resource)?;
        let row = &view.checked_control_rows.uses[ordinal];
        let mapped = row
            .output
            .ok_or(Error::Invalid("control checked Return use omitted"))?;
        if row.input.value != *input_value
            || mapped.coordinate
                != (Use::TerminatorOperand {
                    block: segment.placement.output,
                    operand,
                })
            || mapped.value != *output_value
        {
            return Err(Error::Invalid(
                "control actual Return operand occurrence differs",
            ));
        }
    }
    Ok(())
}

fn source_output_control_segment_edges_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
    segments: &[SourceOutputProjectionSegmentV1],
    segment: SourceOutputProjectionSegmentV1,
    arguments: &[SourceOutputProjectionArgumentV1],
    context: &mut SourceOutputControlNormalizationV1<'_, '_, '_, '_, '_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use SourceOutputControlSelectionV1 as Selection;
    context
        .inner
        .paid(7)
        .ok_or_else(|| context.inner.failure())?;
    let before = view
        .source
        .executable()
        .module()
        .functions
        .get(segment.original.function.0 as usize)
        .and_then(|function| function.body.as_ref())
        .and_then(|body| body.blocks.get(segment.original.block as usize))
        .ok_or(Error::Invalid("control original block absent"))?;
    let tail = &candidate.lowering.kernel().blocks()[segment.claim.tail as usize];
    let edge_count = match before
        .terminator
        .as_ref()
        .ok_or(Error::Invalid("control original terminator absent"))?
    {
        Terminator::Branch { .. } => 1,
        Terminator::ConditionalBranch { .. } => 2,
        Terminator::Switch { cases, .. } if cases.len() == 1 => 2,
        Terminator::Return { values } => {
            if segment.claim.end != segment.claim.tail + 1
                || !matches!(tail.terminator(), ProductionRankedTerminatorV1::Return)
            {
                return Err(Error::Invalid("control actual Return boundary differs"));
            }
            return source_output_control_return_v1(view, segment, values, context.inner.budget);
        }
        _ => {
            return Err(Error::Invalid(
                "control original terminator expansion unsupported",
            ));
        }
    };
    let mut live = [None, None];
    let mut count = 0;
    for successor in 0..edge_count {
        let edge = source_output_control_edge_v1(
            &view.checked_control_rows,
            fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1 {
                source: segment.original,
                successor,
            },
            context.inner.budget,
        )?;
        source_output_control_argument_rows_v1(
            &view.checked_control_rows,
            edge,
            context.inner.budget,
        )?;
        if edge.checked.executable {
            live[count] = Some(edge);
            count += 1;
        }
    }
    match count {
        1 => {
            let edge = live[0].ok_or(Error::Invalid("control selected edge absent"))?;
            match tail.terminator() {
                ProductionRankedTerminatorV1::Branch { target }
                    if segment.claim.end == segment.claim.tail + 1 =>
                {
                    source_output_control_target_v1(
                        view,
                        candidate,
                        segments,
                        edge,
                        *target,
                        context.inner.budget,
                    )
                }
                ProductionRankedTerminatorV1::Trap
                    if segment.claim.end == segment.claim.tail + 1 =>
                {
                    source_output_control_trap_v1(view, candidate, edge, context.inner.budget)
                }
                _ => Err(Error::Invalid(
                    "control selected edge did not become its exact continuation",
                )),
            }
        }
        2 => {
            let first = live[0].ok_or(Error::Invalid("control first edge absent"))?;
            let second = live[1].ok_or(Error::Invalid("control second edge absent"))?;
            let (condition, true_edge, false_edge) = match (&first.output, &second.output) {
                (Some(a), Some(b)) => match (&a.selection, &b.selection) {
                    (
                        Selection::Boolean {
                            value: x,
                            expected: true,
                        },
                        Selection::Boolean {
                            value: y,
                            expected: false,
                        },
                    ) if x == y && a.coordinate.source == b.coordinate.source => {
                        (*x, first, second)
                    }
                    (
                        Selection::Boolean {
                            value: x,
                            expected: false,
                        },
                        Selection::Boolean {
                            value: y,
                            expected: true,
                        },
                    ) if x == y && a.coordinate.source == b.coordinate.source => {
                        (*x, second, first)
                    }
                    _ => {
                        return Err(Error::Invalid(
                            "control dynamic edge occurrence or polarity unsupported",
                        ));
                    }
                },
                _ => {
                    return Err(Error::Invalid(
                        "control dynamic edge is not physically retained",
                    ));
                }
            };
            let (true_block, false_block) = match tail.terminator() {
                ProductionRankedTerminatorV1::IndexLessThan {
                    true_block,
                    false_block,
                    ..
                }
                | ProductionRankedTerminatorV1::IndexEqual {
                    true_block,
                    false_block,
                    ..
                } => (*true_block, *false_block),
                _ => {
                    return Err(Error::Invalid(
                        "control dynamic edge was deduplicated or replaced by an analysis split",
                    ));
                }
            };
            context.guard = Some((segment.source, condition));
            source_output_control_predicate_v1(condition, tail.terminator(), arguments, context)?;
            context.guard = None;
            source_output_control_target_v1(
                view,
                candidate,
                segments,
                true_edge,
                true_block,
                context.inner.budget,
            )?;
            source_output_control_target_v1(
                view,
                candidate,
                segments,
                false_edge,
                false_block,
                context.inner.budget,
            )?;
            if segment.claim.end == segment.claim.tail + 2
                && true_block != segment.claim.tail + 1
                && false_block != segment.claim.tail + 1
            {
                return Err(Error::Invalid(
                    "control extra projected trap is not a mapped edge",
                ));
            }
            Ok(())
        }
        _ => Err(Error::Invalid(
            "control live source has no executable successor",
        )),
    }
}

fn source_output_control_leaf_rows_v1<'a>(
    arguments: &'a [SourceOutputProjectionArgumentV1],
    literal_uses: &'a [SourceOutputProjectionLiteralUseV1],
    range: std::ops::Range<usize>,
    ranked_value: ProductionRankedValueV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<Option<SourceOutputAddressLeafV1<'a>>, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(1).map_err(Error::Resource)?;
    let arguments = arguments
        .get(range)
        .ok_or(Error::Invalid("control argument span absent"))?;
    let ordinal = assert_origin_find_v1(arguments, budget, |row, budget| {
        budget.charge_work(1)?;
        Ok(row.ranked_value.cmp(&ranked_value))
    })
    .map_err(Error::SourceOrigin)?;
    let Some(ordinal) = ordinal else {
        return Ok(None);
    };
    let row = &arguments[ordinal];
    Ok(Some(match &row.origin {
        SourceOutputProjectionLeafOriginV1::Formal(formal) => {
            SourceOutputAddressLeafV1::Formal(ProductionConditionalMemoryArgumentV1 { row, formal })
        }
        SourceOutputProjectionLeafOriginV1::Literal(literal) => {
            SourceOutputAddressLeafV1::Literal(ProductionConditionalMemoryLiteralV1 {
                row,
                literal,
                uses: literal_uses
                    .get(literal.first_use..literal.end_use)
                    .ok_or(Error::Invalid("literal guard-use span absent"))?,
            })
        }
        SourceOutputProjectionLeafOriginV1::Invocation(invocation) => {
            if invocation.identity_getter {
                return Err(Error::Invalid(
                    "identity getter address join is not implemented",
                ));
            }
            SourceOutputAddressLeafV1::Invocation(row, invocation)
        }
    }))
}

type SourceOutputInvocationEventKeyV1 = (u32, u32, u32, u32, u32, u32, u32);

#[derive(Clone, Copy)]
enum SourceOutputInvocationDefinitionV1 {
    Event(usize),
    Edge(usize),
}

struct SourceOutputInvocationSourceIndexV1 {
    source: *const (),
    function: SemanticFunctionIdV1,
    canonical: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    events: Vec<(SourceOutputInvocationEventKeyV1, usize)>,
    definitions: Vec<(SsaValueV1, SourceOutputInvocationDefinitionV1)>,
    incoming: Vec<((u32, fe2o3_mir_model::SsaEdgeIdV1), usize)>,
    transports: Vec<((fe2o3_mir_model::SsaEdgeIdV1, u32), SsaValueV1)>,
    statements: Vec<((u32, u32), usize)>,
    terminators: Vec<(u32, usize)>,
    blocks: Vec<(BlockId, u32)>,
    values: Vec<(ValueId, (u32, u32, u32))>,
}

fn source_output_invocation_event_key_v1(
    event: &fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1,
) -> Option<SourceOutputInvocationEventKeyV1> {
    use fe2o3_pliron::{
        ProductionSemanticSsaEventRoleV1 as Role, ProductionSemanticSsaOccurrenceSiteV1 as Site,
        ProductionSemanticSsaOperandRoleV1 as Operand,
    };
    let (kind, block, statement) = match event.site() {
        Site::Statement { block, statement } => (0, block.get(), statement),
        Site::Terminator { block } => (1, block.get(), 0),
    };
    let (operand, argument) = match event.operand() {
        Operand::RvalueOperand(argument) => (0, argument),
        Operand::RvaluePlace => (1, 0),
        Operand::Destination => (2, 0),
        Operand::CallArgument(argument) => (3, argument),
        Operand::AssertMessage(argument) => (4, argument),
        _ => return None,
    };
    let (role, projection) = match event.role() {
        Role::BaseUse => (0, 0),
        Role::ProjectionIndexUse(projection) => (1, projection),
        Role::DestinationDefine => (2, 0),
        _ => return None,
    };
    Some((kind, block, statement, operand, argument, role, projection))
}

fn source_output_invocation_capture_v1<'a>(
    source: &'a crate::ProductionPreRankedKirOwnerV1,
    function: SemanticFunctionIdV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<
    fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'a>,
    ProductionSourceOutputErrorV1,
> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(8).map_err(Error::Resource)?;
    let ssa = source.semantic_ssa();
    let receipt = ssa
        .occurrence_storage()
        .ok_or(Error::Invalid("invocation source capture receipt absent"))?;
    let captured = ssa
        .occurrences_v1()
        .and_then(|view| view.function(function))
        .ok_or(Error::Invalid("invocation source capture absent"))?;
    if !std::ptr::eq(captured.owner(), ssa) || budget.storage() < receipt.retained_storage() {
        return Err(Error::Invalid("invocation source capture custody differs"));
    }
    Ok(captured)
}

fn source_output_invocation_sort_v1<T>(
    rows: &mut [T],
    fields: usize,
    unique: bool,
    compare: impl Fn(&T, &T) -> std::cmp::Ordering,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    assert_origin_sort_v1(rows, budget, |a, b, budget| {
        budget.charge_work(fields)?;
        Ok(compare(a, b))
    })
    .map_err(Error::SourceOrigin)?;
    if unique {
        for pair in rows.windows(2) {
            budget.charge_work(fields).map_err(Error::Resource)?;
            if compare(&pair[0], &pair[1]).is_eq() {
                return Err(Error::Invalid("invocation source index key duplicated"));
            }
        }
    }
    Ok(())
}

fn source_output_invocation_source_index_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
    canonical: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputInvocationSourceIndexV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let captured =
        source_output_invocation_capture_v1(view.source, candidate.selected_function, budget)?;
    budget
        .reserve_storage(std::mem::size_of::<SourceOutputInvocationSourceIndexV1>())
        .map_err(Error::Resource)?;
    let plan = view
        .source
        .semantic_ssa()
        .plan_for_function(candidate.selected_function)
        .ok_or(Error::Invalid("invocation source SSA plan absent"))?
        .plan();
    let mut index = SourceOutputInvocationSourceIndexV1 {
        source: std::ptr::from_ref(view.source).cast(),
        function: candidate.selected_function,
        canonical,
        events: Vec::new(),
        definitions: Vec::new(),
        incoming: Vec::new(),
        transports: Vec::new(),
        statements: Vec::new(),
        terminators: Vec::new(),
        blocks: Vec::new(),
        values: Vec::new(),
    };
    for (ordinal, event) in captured.events().iter().enumerate() {
        budget.charge_work(8).map_err(Error::Resource)?;
        if let Some(key) = source_output_invocation_event_key_v1(event) {
            assert_origin_push_v1(&mut index.events, (key, ordinal), budget)
                .map_err(Error::SourceOrigin)?;
        }
        if event.is_reachable()
            && event.is_promoted()
            && let Some(SsaResolvedEventV1::Define { value, .. }) = event.resolved()
        {
            assert_origin_push_v1(
                &mut index.definitions,
                (value, SourceOutputInvocationDefinitionV1::Event(ordinal)),
                budget,
            )
            .map_err(Error::SourceOrigin)?;
        }
    }
    for (ordinal, definition) in captured.edge_definitions().iter().enumerate() {
        budget.charge_work(6).map_err(Error::Resource)?;
        if definition.is_reachable()
            && definition.is_promoted()
            && let Some(value) = definition.value()
        {
            assert_origin_push_v1(
                &mut index.definitions,
                (value, SourceOutputInvocationDefinitionV1::Edge(ordinal)),
                budget,
            )
            .map_err(Error::SourceOrigin)?;
        }
    }
    for (ordinal, successor) in captured.successors().iter().enumerate() {
        budget.charge_work(4).map_err(Error::Resource)?;
        if !plan.is_reachable(successor.id().source()) {
            continue;
        }
        assert_origin_push_v1(
            &mut index.incoming,
            ((successor.edge().target().index(), successor.id()), ordinal),
            budget,
        )
        .map_err(Error::SourceOrigin)?;
        for argument in plan
            .edge_arguments(successor.id())
            .ok_or(Error::Invalid("invocation source edge arguments absent"))?
        {
            budget.charge_work(4).map_err(Error::Resource)?;
            assert_origin_push_v1(
                &mut index.transports,
                (
                    (successor.id(), argument.variable().get()),
                    argument.value(),
                ),
                budget,
            )
            .map_err(Error::SourceOrigin)?;
        }
    }
    for (ordinal, span) in view
        .source
        .correspondence
        .statement_operation_spans()
        .iter()
        .enumerate()
    {
        budget.charge_work(5).map_err(Error::Resource)?;
        if span.correspondence_owner() == candidate.selected_root
            && span.semantic_function() == candidate.selected_function
        {
            assert_origin_push_v1(
                &mut index.statements,
                (
                    (span.semantic_block().index(), span.statement_ordinal()),
                    ordinal,
                ),
                budget,
            )
            .map_err(Error::SourceOrigin)?;
        }
    }
    for (ordinal, span) in view
        .source
        .correspondence
        .terminator_operation_spans()
        .iter()
        .enumerate()
    {
        budget.charge_work(5).map_err(Error::Resource)?;
        if span.correspondence_owner() == candidate.selected_root
            && span.semantic_function() == candidate.selected_function
        {
            assert_origin_push_v1(
                &mut index.terminators,
                (span.semantic_block().index(), ordinal),
                budget,
            )
            .map_err(Error::SourceOrigin)?;
        }
    }
    let body = view
        .source
        .executable()
        .module()
        .functions
        .get(canonical.0 as usize)
        .and_then(|function| function.body.as_ref())
        .ok_or(Error::Invalid("invocation N body absent"))?;
    for (block, row) in body.blocks.iter().enumerate() {
        budget.charge_work(2).map_err(Error::Resource)?;
        let block = u32::try_from(block)
            .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        assert_origin_push_v1(&mut index.blocks, (row.id, block), budget)
            .map_err(Error::SourceOrigin)?;
        for (operation, row) in row.operations.iter().enumerate() {
            budget.charge_work(2).map_err(Error::Resource)?;
            let operation = u32::try_from(operation)
                .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            for (result, value) in row.results.iter().enumerate() {
                budget.charge_work(3).map_err(Error::Resource)?;
                let result = u32::try_from(result)
                    .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?;
                assert_origin_push_v1(
                    &mut index.values,
                    (value.id, (block, operation, result)),
                    budget,
                )
                .map_err(Error::SourceOrigin)?;
            }
        }
    }
    source_output_invocation_sort_v1(&mut index.events, 7, true, |a, b| a.0.cmp(&b.0), budget)?;
    source_output_invocation_sort_v1(
        &mut index.definitions,
        4,
        true,
        |a, b| a.0.cmp(&b.0),
        budget,
    )?;
    source_output_invocation_sort_v1(&mut index.incoming, 3, true, |a, b| a.0.cmp(&b.0), budget)?;
    source_output_invocation_sort_v1(&mut index.transports, 3, true, |a, b| a.0.cmp(&b.0), budget)?;
    source_output_invocation_sort_v1(&mut index.statements, 2, true, |a, b| a.0.cmp(&b.0), budget)?;
    source_output_invocation_sort_v1(
        &mut index.terminators,
        1,
        true,
        |a, b| a.0.cmp(&b.0),
        budget,
    )?;
    source_output_invocation_sort_v1(&mut index.blocks, 1, true, |a, b| a.0.cmp(&b.0), budget)?;
    source_output_invocation_sort_v1(&mut index.values, 1, true, |a, b| a.0.cmp(&b.0), budget)?;
    Ok(index)
}

fn source_output_invocation_use_v1(
    index: &SourceOutputInvocationSourceIndexV1,
    captured: &fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    key: SourceOutputInvocationEventKeyV1,
    local: SemanticLocalIdV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SsaValueV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let row = assert_origin_find_v1(&index.events, budget, |row, budget| {
        budget.charge_work(7)?;
        Ok(row.0.cmp(&key))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid("invocation exact source use absent"))?;
    budget.charge_work(8).map_err(Error::Resource)?;
    let event = captured
        .events()
        .get(index.events[row].1)
        .ok_or(Error::Invalid("invocation source use ordinal differs"))?;
    if source_output_invocation_event_key_v1(event) != Some(key)
        || !event.is_reachable()
        || !event.is_promoted()
    {
        return Err(Error::Invalid(
            "invocation source use is not live promoted exact occurrence",
        ));
    }
    match (event.event(), event.resolved()) {
        (
            fe2o3_mir_model::SsaEventV1::Use(original),
            Some(SsaResolvedEventV1::Use { variable, value }),
        ) if original == variable && variable.get() == local.index() => Ok(value),
        _ => Err(Error::Invalid("invocation captured source operand differs")),
    }
}

fn source_output_invocation_definition_v1(
    index: &SourceOutputInvocationSourceIndexV1,
    mut value: SsaValueV1,
    local: SemanticLocalIdV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(SsaValueV1, SourceOutputInvocationDefinitionV1), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    source_output_global_scratch_scope_v1(budget, |budget| {
        budget
            .reserve_storage(std::mem::size_of::<
                [Option<SsaValueV1>; MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
            >())
            .map_err(Error::Resource)?;
        let mut visited = [None; MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1];
        for depth in 0..visited.len() {
            budget.charge_work(depth + 16).map_err(Error::Resource)?;
            if visited[..depth].contains(&Some(value)) {
                return Err(Error::Invalid("invocation source forwarding cycle"));
            }
            visited[depth] = Some(value);
            let SsaValueV1::BlockArgument { block, variable } = value else {
                let row = assert_origin_find_v1(&index.definitions, budget, |row, budget| {
                    budget.charge_work(4)?;
                    Ok(row.0.cmp(&value))
                })
                .map_err(Error::SourceOrigin)?
                .ok_or(Error::Invalid("invocation source definition absent"))?;
                return Ok(index.definitions[row]);
            };
            if variable.get() != local.index() {
                return Err(Error::Invalid("invocation forwarded variable differs"));
            }
            let row = assert_origin_find_v1(&index.incoming, budget, |row, budget| {
                budget.charge_work(1)?;
                Ok(row.0.0.cmp(&block.get()))
            })
            .map_err(Error::SourceOrigin)?
            .ok_or(Error::Invalid("invocation source incoming edge absent"))?;
            budget.charge_work(4).map_err(Error::Resource)?;
            if row
                .checked_sub(1)
                .and_then(|i| index.incoming.get(i))
                .is_some_and(|other| other.0.0 == block.get())
                || index
                    .incoming
                    .get(row + 1)
                    .is_some_and(|other| other.0.0 == block.get())
            {
                return Err(Error::Invalid(
                    "invocation source has multiple incoming edges",
                ));
            }
            let key = (index.incoming[row].0.1, variable.get());
            let row = assert_origin_find_v1(&index.transports, budget, |row, budget| {
                budget.charge_work(3)?;
                Ok(row.0.cmp(&key))
            })
            .map_err(Error::SourceOrigin)?
            .ok_or(Error::Invalid("invocation same-variable transport absent"))?;
            value = index.transports[row].1;
        }
        Err(Error::Invalid(
            "invocation source forwarding depth exceeded",
        ))
    })
}

fn source_output_invocation_statement_span_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    index: &SourceOutputInvocationSourceIndexV1,
    block: u32,
    statement: u32,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let key = (block, statement);
    let row = assert_origin_find_v1(&index.statements, budget, |row, budget| {
        budget.charge_work(2)?;
        Ok(row.0.cmp(&key))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid("invocation source alias span absent"))?;
    budget.charge_work(4).map_err(Error::Resource)?;
    let span = view.source.correspondence.statement_operation_spans()[index.statements[row].1];
    if span.operation_count() != 0 {
        return Err(Error::Invalid("invocation source alias emits operations"));
    }
    Ok(())
}

fn source_output_invocation_copy_origin_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    index: &SourceOutputInvocationSourceIndexV1,
    captured: &fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    mut local: SemanticLocalIdV1,
    mut value: SsaValueV1,
    ty: fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<
    (
        SemanticLocalIdV1,
        SsaValueV1,
        SourceOutputInvocationDefinitionV1,
    ),
    ProductionSourceOutputErrorV1,
> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1 as Site;
    let function =
        &view.source.semantic_ssa().source_semantic().functions()[index.function.index() as usize];
    for _ in 0..=MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        budget.charge_work(16).map_err(Error::Resource)?;
        if function
            .locals()
            .get(local.index() as usize)
            .is_none_or(|row| row.ty() != ty)
        {
            return Err(Error::Invalid("invocation source alias type differs"));
        }
        let (resolved, definition) =
            source_output_invocation_definition_v1(index, value, local, budget)?;
        let SourceOutputInvocationDefinitionV1::Event(ordinal) = definition else {
            return Ok((local, resolved, definition));
        };
        let event = &captured.events()[ordinal];
        let Site::Statement { block, statement } = event.site() else {
            return Err(Error::Invalid(
                "invocation source event definition site unsupported",
            ));
        };
        if !matches!((event.event(), event.resolved()),
            (fe2o3_mir_model::SsaEventV1::Define(original), Some(SsaResolvedEventV1::Define { variable, value }))
            if original == variable && variable.get() == local.index() && value == resolved)
            || source_output_invocation_event_key_v1(event)
                != Some((0, block.get(), statement, 2, 0, 2, 0))
        {
            return Err(Error::Invalid(
                "invocation source destination definition differs",
            ));
        }
        let row = function
            .blocks()
            .get(block.get() as usize)
            .and_then(|block| block.statements().get(statement as usize))
            .ok_or(Error::Invalid(
                "invocation source defining statement absent",
            ))?;
        let SemanticStatementKindV1::Assign(assignment) = row.kind() else {
            return Err(Error::Invalid(
                "invocation source definition is not an assignment",
            ));
        };
        if assignment.destination().local() != local
            || !assignment.destination().projections().is_empty()
            || assignment.destination().ty() != ty
            || assignment.value().result_type() != ty
        {
            return Err(Error::Invalid(
                "invocation source assignment type or destination differs",
            ));
        }
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = assignment.value().kind()
        else {
            return Ok((local, resolved, definition));
        };
        if !place.projections().is_empty() || place.ty() != ty {
            return Err(Error::Invalid(
                "invocation source Copy is not typed plain transport",
            ));
        }
        source_output_invocation_statement_span_v1(view, index, block.get(), statement, budget)?;
        local = place.local();
        value = source_output_invocation_use_v1(
            index,
            captured,
            (0, block.get(), statement, 0, 0, 0, 0),
            local,
            budget,
        )?;
    }
    Err(Error::Invalid("invocation source Copy depth exceeded"))
}

fn source_output_invocation_call_v1<'a>(
    function: &'a SemanticFunctionDeclV1,
    index: &SourceOutputInvocationSourceIndexV1,
    captured: &fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    local: SemanticLocalIdV1,
    value: SsaValueV1,
    definition: SourceOutputInvocationDefinitionV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<
    (
        fe2o3_mir_model::SsaEdgeIdV1,
        &'a fe2o3_mir_model::semantic_mir_v1::SemanticDirectCallV1,
    ),
    ProductionSourceOutputErrorV1,
> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(16).map_err(Error::Resource)?;
    let SourceOutputInvocationDefinitionV1::Edge(ordinal) = definition else {
        return Err(Error::Invalid(
            "invocation result lacks a source CallReturn definition",
        ));
    };
    let row = captured
        .edge_definitions()
        .get(ordinal)
        .ok_or(Error::Invalid("invocation edge definition absent"))?;
    if row.variable().get() != local.index()
        || row.value() != Some(value)
        || row.ordinal() != 0
        || !row.is_reachable()
        || !row.is_promoted()
    {
        return Err(Error::Invalid(
            "invocation source CallReturn definition differs",
        ));
    }
    let block = function
        .blocks()
        .get(row.edge().source().get() as usize)
        .ok_or(Error::Invalid("invocation source Call block absent"))?;
    let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
        return Err(Error::Invalid("invocation source definition is not a Call"));
    };
    let destination = call
        .destination()
        .ok_or(Error::Invalid("invocation source Call has no destination"))?;
    if !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable)
        || destination.place().local() != local
        || !destination.place().projections().is_empty()
        || destination.edge().role()
            != fe2o3_mir_model::semantic_mir_v1::SemanticEdgeRoleV1::CallReturn
    {
        return Err(Error::Invalid(
            "invocation source Call continuation differs",
        ));
    }
    // The sole normal edge is occurrence zero; cleanup and no-destination calls
    // were refused above, so a target-only match cannot authenticate this row.
    if row.edge().ordinal() != 0 {
        return Err(Error::Invalid(
            "invocation source Call successor ordinal differs",
        ));
    }
    let key = (destination.edge().target().index(), row.edge());
    let found = assert_origin_find_v1(&index.incoming, budget, |row, budget| {
        budget.charge_work(3)?;
        Ok(row.0.cmp(&key))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid("invocation captured CallReturn differs"))?;
    let edge = &captured.successors()[index.incoming[found].1];
    if edge.id() != row.edge() || edge.edge() != destination.edge() {
        return Err(Error::Invalid(
            "invocation captured CallReturn role differs",
        ));
    }
    Ok((row.edge(), call))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceOutputInvocationSourceAnchorV1 {
    get: fe2o3_mir_model::SsaEdgeIdV1,
    producer: fe2o3_mir_model::SsaEdgeIdV1,
    raw: SsaValueV1,
    witness: SsaValueV1,
}

fn source_output_invocation_source_anchor_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    index: &SourceOutputInvocationSourceIndexV1,
    local: SemanticLocalIdV1,
    value: SsaValueV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputInvocationSourceAnchorV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticBorrowKindV1, SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1,
        SemanticMutabilityV1, SemanticPointerKindV1, SemanticPointerMetadataV1,
    };
    use fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1 as Site;
    budget.charge_work(4).map_err(Error::Resource)?;
    if index.source != std::ptr::from_ref(view.source).cast() {
        return Err(Error::Invalid("invocation indexed source owner differs"));
    }
    let captured = source_output_invocation_capture_v1(view.source, index.function, budget)?;
    let semantic = view.source.semantic_ssa().source_semantic();
    let function = semantic
        .functions()
        .get(index.function.index() as usize)
        .ok_or(Error::Invalid("invocation indexed source function absent"))?;
    let raw = function
        .locals()
        .get(local.index() as usize)
        .ok_or(Error::Invalid("invocation raw source local absent"))?
        .ty();
    budget.charge_work(8).map_err(Error::Resource)?;
    if !matches!(
        semantic
            .types()
            .get(raw.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64
        }))
    ) || lower_scalar_type(semantic.types(), raw).map_err(Error::SourceReplay)?
        != Type::Scalar(ScalarType::U64)
    {
        return Err(Error::Invalid(
            "invocation source raw index is not unsigned 64-bit",
        ));
    }
    let (raw_local, raw_value, definition) =
        source_output_invocation_copy_origin_v1(view, index, &captured, local, value, raw, budget)?;
    let (get, call) = source_output_invocation_call_v1(
        function, index, &captured, raw_local, raw_value, definition, budget,
    )?;
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        operation:
            SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
                index_witness,
                raw_index,
            },
        ..
    }) = semantic.callables().get(call.callee().index() as usize)
    else {
        return Err(Error::Invalid(
            "invocation raw definition is not authenticated ThreadIndexGet",
        ));
    };
    budget
        .charge_work(call.arguments().len())
        .map_err(Error::Resource)?;
    let [SemanticOperandV1::Copy(reference)] = call.arguments() else {
        return Err(Error::Invalid(
            "invocation Get requires one copied typed reference",
        ));
    };
    if *raw_index != raw
        || !reference.projections().is_empty()
        || call
            .destination()
            .is_none_or(|destination| destination.place().ty() != raw)
    {
        return Err(Error::Invalid("invocation Get source types differ"));
    }
    let Some(SemanticTypeShapeV1::Pointer(pointer)) = semantic
        .types()
        .get(reference.ty().index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Err(Error::Invalid(
            "invocation Get argument is not a source reference",
        ));
    };
    if pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.mutability() != SemanticMutabilityV1::Immutable
        || pointer.pointee() != *index_witness
        || pointer.metadata() != SemanticPointerMetadataV1::None
    {
        return Err(Error::Invalid(
            "invocation Get witness reference type differs",
        ));
    }
    let reference_value = source_output_invocation_use_v1(
        index,
        &captured,
        (1, get.source().get(), 0, 3, 0, 0, 0),
        reference.local(),
        budget,
    )?;
    let (_, _, definition) = source_output_invocation_copy_origin_v1(
        view,
        index,
        &captured,
        reference.local(),
        reference_value,
        reference.ty(),
        budget,
    )?;
    let SourceOutputInvocationDefinitionV1::Event(ordinal) = definition else {
        return Err(Error::Invalid(
            "invocation Get reference lacks an actual shared Borrow",
        ));
    };
    let Site::Statement { block, statement } = captured.events()[ordinal].site() else {
        return Err(Error::Invalid("invocation Borrow source site differs"));
    };
    let SemanticStatementKindV1::Assign(assignment) =
        function.blocks()[block.get() as usize].statements()[statement as usize].kind()
    else {
        return Err(Error::Invalid("invocation Borrow source assignment absent"));
    };
    let SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Shared,
        place,
    } = assignment.value().kind()
    else {
        return Err(Error::Invalid(
            "invocation Get receiver was not a shared Borrow",
        ));
    };
    if !place.projections().is_empty() || place.ty() != *index_witness {
        return Err(Error::Invalid("invocation Borrow witness type differs"));
    }
    source_output_invocation_statement_span_v1(view, index, block.get(), statement, budget)?;
    let witness = source_output_invocation_use_v1(
        index,
        &captured,
        (0, block.get(), statement, 1, 0, 0, 0),
        place.local(),
        budget,
    )?;
    let (witness_local, witness_value, definition) = source_output_invocation_copy_origin_v1(
        view,
        index,
        &captured,
        place.local(),
        witness,
        *index_witness,
        budget,
    )?;
    let (producer, call) = source_output_invocation_call_v1(
        function,
        index,
        &captured,
        witness_local,
        witness_value,
        definition,
        budget,
    )?;
    budget
        .charge_work(call.arguments().len())
        .map_err(Error::Resource)?;
    if !call.arguments().is_empty()
        || call
            .destination()
            .is_none_or(|destination| destination.place().ty() != *index_witness)
        || !matches!(semantic.callables().get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {index_witness: actual, raw_index: raw}, ..
            }) if actual == index_witness && raw == raw_index)
    {
        return Err(Error::Invalid(
            "invocation witness is not authenticated ThreadIndex1d",
        ));
    }
    Ok(SourceOutputInvocationSourceAnchorV1 {
        get,
        producer,
        raw: raw_value,
        witness: witness_value,
    })
}

fn source_output_invocation_n_root_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    index: &SourceOutputInvocationSourceIndexV1,
    source: SourceOutputInvocationSourceAnchorV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<
    (fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1, ValueId),
    ProductionSourceOutputErrorV1,
> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::{
        CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Def,
        CanonicalKirOperationCoordinateV1 as Op,
    };
    let body = view.source.executable().module().functions[index.canonical.0 as usize]
        .body
        .as_ref()
        .ok_or(Error::Invalid("invocation N body absent"))?;
    let mut result = None;
    for (edge, count) in [(source.get, 0), (source.producer, 1)] {
        let found = assert_origin_find_v1(&index.terminators, budget, |row, budget| {
            budget.charge_work(1)?;
            Ok(row.0.cmp(&edge.source().get()))
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("invocation N Call span absent"))?;
        let span =
            view.source.correspondence.terminator_operation_spans()[index.terminators[found].1];
        budget.charge_work(8).map_err(Error::Resource)?;
        if span.operation_count() != count {
            return Err(Error::Invalid(
                "invocation N Call span is not exact intrinsic or alias",
            ));
        }
        let found = assert_origin_find_v1(&index.blocks, budget, |row, budget| {
            budget.charge_work(1)?;
            Ok(row.0.cmp(&span.kernel_ir_block()))
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("invocation N span block absent"))?;
        let block = index.blocks[found].1;
        if !matches!(
            body.blocks[block as usize].terminator,
            Some(Terminator::Branch { .. })
        ) {
            return Err(Error::Invalid("invocation N CallReturn is not one branch"));
        }
        if count == 0 {
            continue;
        }
        let operation = body.blocks[block as usize]
            .operations
            .get(span.first_operation_ordinal() as usize)
            .ok_or(Error::Invalid("invocation N producer operation absent"))?;
        if operation.results.len() != 1
            || operation.results[0].ty != Type::INDEX
            || !matches!(&operation.kind, OperationKind::Intrinsic(intrinsic)
                if *intrinsic == fe2o3_kernel_ir::IntrinsicOperation::global_id_1d())
        {
            return Err(Error::Invalid(
                "invocation N producer is not exact GlobalX INDEX",
            ));
        }
        result = Some((
            Def::Result {
                operation: Op {
                    block: Block {
                        function: index.canonical,
                        block,
                    },
                    operation: span.first_operation_ordinal(),
                },
                result: 0,
            },
            operation.results[0].id,
        ));
    }
    result.ok_or(Error::Invalid("invocation N producer absent"))
}

fn source_output_invocation_n_ancestry_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    index: &SourceOutputInvocationSourceIndexV1,
    mut value: ValueId,
    root: ValueId,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let body = view.source.executable().module().functions[index.canonical.0 as usize]
        .body
        .as_ref()
        .ok_or(Error::Invalid("invocation N body absent"))?;
    for _ in 0..=MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        budget.charge_work(4).map_err(Error::Resource)?;
        if value == root {
            return Ok(());
        }
        let found = assert_origin_find_v1(&index.values, budget, |row, budget| {
            budget.charge_work(1)?;
            Ok(row.0.cmp(&value))
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("invocation N use definition absent"))?;
        let (block, ordinal, result) = index.values[found].1;
        let operation = &body.blocks[block as usize].operations[ordinal as usize];
        budget.charge_work(8).map_err(Error::Resource)?;
        let OperationKind::Cast {
            kind: CastKind::Bitcast,
            value: operand,
            to,
        } = &operation.kind
        else {
            return Err(Error::Invalid(
                "invocation N ancestry is not identity transport",
            ));
        };
        if result != 0
            || operation.results.len() != 1
            || operation.results[0].id != value
            || kir_semantic_scalar_v1(&operation.results[0].ty)
                != Some(source_output_address_u64_v1())
            || kir_semantic_scalar_v1(to) != Some(source_output_address_u64_v1())
        {
            return Err(Error::Invalid(
                "invocation N identity width or result differs",
            ));
        }
        value = *operand;
    }
    Err(Error::Invalid("invocation N ancestry depth exceeded"))
}

fn source_output_invocation_output_root_v1(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    canonical: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    mut value: ValueId,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<
    (fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1, ValueId),
    ProductionSourceOutputErrorV1,
> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Def;
    budget.charge_work(2).map_err(Error::Resource)?;
    let function = inventory
        .functions()
        .get(canonical.0 as usize)
        .ok_or(Error::Invalid("invocation O function absent"))?;
    for _ in 0..=MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        let definition = inventory
            .definition_for_value(canonical, value, budget)
            .map_err(Error::Inventory)?
            .ok_or(Error::Invalid("invocation O definition absent"))?;
        budget.charge_work(8).map_err(Error::Resource)?;
        if kir_semantic_scalar_v1(definition.ty) != Some(source_output_address_u64_v1()) {
            return Err(Error::Invalid("invocation O coordinate width differs"));
        }
        let Def::Result {
            operation,
            result: 0,
        } = definition.coordinate
        else {
            return Err(Error::Invalid("invocation O coordinate is not result zero"));
        };
        let row = source_output_address_operation_v1(inventory, function, operation, budget)?;
        if row.operation.results.len() != 1 || row.operation.results[0].id != value {
            return Err(Error::Invalid("invocation O result identity differs"));
        }
        match &row.operation.kind {
            OperationKind::Intrinsic(intrinsic)
                if *intrinsic == fe2o3_kernel_ir::IntrinsicOperation::global_id_1d()
                    && row.operation.results[0].ty == Type::INDEX =>
            {
                return Ok((definition.coordinate, value));
            }
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: operand,
                to,
            } if kir_semantic_scalar_v1(to) == Some(source_output_address_u64_v1()) => {
                source_output_address_operand_v1(inventory, row, 0, *operand, budget)?;
                value = *operand;
            }
            _ => {
                return Err(Error::Invalid(
                    "invocation O ancestry is not sealed GlobalX transport",
                ));
            }
        }
    }
    Err(Error::Invalid("invocation O ancestry depth exceeded"))
}

fn source_output_invocation_symbol_slot_v1(
    parameters: usize,
) -> Result<u32, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let slot = u32::try_from(parameters)
        .ok()
        .and_then(|n| n.checked_mul(2))
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2
        .checked_add(slot)
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    Ok(slot)
}

fn source_output_invocation_symbol_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
    canonical: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<u32, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget
        .charge_work(view.source.source_launch.roots().len())
        .map_err(Error::Resource)?;
    let root = view
        .source
        .source_launch
        .roots()
        .iter()
        .find(|root| root.selected_root() == candidate.selected_root)
        .ok_or(Error::Invalid("invocation source launch root absent"))?;
    budget.charge_work(81).map_err(Error::Resource)?;
    let source = &view.source.semantic_ssa().source_semantic().functions()
        [candidate.selected_root.index() as usize];
    let entry = source
        .kernel_entry()
        .ok_or(Error::Invalid("invocation source entry absent"))?;
    let layout = root.layout();
    let expected = ProductionRankedOperationV1::ExecutionLayout {
        grid_identity: layout.grid_identity(),
        global_extents: layout.global_extents(),
        workgroup_extents: layout.workgroup_extents(),
        subgroup_size: layout.subgroup_size(),
        full_physical_workgroups: layout.full_physical_workgroups(),
    };
    if candidate.selected_root != candidate.selected_function
        || source.role() != SemanticFunctionRoleV1::KernelRoot
        || source.identity() != root.semantic_root_identity()
        || entry.kernel_binding_identity().as_bytes() != &root.kernel_binding()
        || root.source_rank() != 1
        || layout.global_extents()[1..] != [1, 1]
        || layout.workgroup_extents()[1..] != [1, 1]
        || candidate
            .lowering
            .kernel()
            .blocks()
            .first()
            .and_then(|b| b.operations().first())
            != Some(&expected)
    {
        return Err(Error::Invalid(
            "invocation exact GlobalX source launch differs",
        ));
    }
    let function = view
        .output()
        .module()
        .functions
        .get(canonical.0 as usize)
        .ok_or(Error::Invalid("invocation output function absent"))?;
    source_output_invocation_symbol_slot_v1(function.signature.parameters.len())
}

#[allow(clippy::too_many_arguments)]
fn source_output_control_invocation_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
    index: &SourceOutputInvocationSourceIndexV1,
    source_index: usize,
    claim: ProductionProjectionArgumentCandidateV1,
    uses: &mut Vec<SourceOutputProjectionLiteralUseV1>,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputProjectionArgumentV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::{
        CanonicalKirDefinitionCoordinateV1 as Def, CanonicalKirUseCoordinateV1 as Use,
    };
    let symbol = source_output_invocation_symbol_v1(view, candidate, index.canonical, budget)?;
    let captured = source_output_invocation_capture_v1(view.source, index.function, budget)?;
    let function =
        &view.source.semantic_ssa().source_semantic().functions()[index.function.index() as usize];
    let body = view.source.executable().module().functions[index.canonical.0 as usize]
        .body
        .as_ref()
        .ok_or(Error::Invalid("invocation original body absent"))?;
    let first_use = uses.len();
    let mut anchor = None;
    for segment in &candidate.control.blocks {
        budget.charge_work(4).map_err(Error::Resource)?;
        let tail = candidate
            .lowering
            .kernel()
            .blocks()
            .get(segment.tail as usize)
            .ok_or(Error::Invalid("invocation projected guard absent"))?;
        let (lhs, rhs) = match tail.terminator() {
            ProductionRankedTerminatorV1::IndexLessThan { lhs, rhs, .. }
            | ProductionRankedTerminatorV1::IndexEqual { lhs, rhs, .. } => (*lhs, *rhs),
            _ => continue,
        };
        if lhs != claim.ranked_value && rhs != claim.ranked_value {
            continue;
        }
        if lhs != claim.ranked_value || rhs == claim.ranked_value {
            return Err(Error::Invalid(
                "invocation requires the exact bounds index operand",
            ));
        }
        let block = function
            .blocks()
            .get(segment.source_block.index() as usize)
            .ok_or(Error::Invalid("invocation source guard absent"))?;
        let SemanticTerminatorKindV1::Assert {
            message:
                SemanticAssertMessageV1::BoundsCheck {
                    index: SemanticOperandV1::Copy(place),
                    ..
                },
            ..
        } = block.terminator().kind()
        else {
            return Err(Error::Invalid(
                "invocation guard is not a copied source bounds index",
            ));
        };
        if place.local() != claim.source_local
            || !place.projections().is_empty()
            || function
                .locals()
                .get(place.local().index() as usize)
                .map(|local| local.ty())
                != Some(place.ty())
        {
            return Err(Error::Invalid(
                "invocation source guard local or type differs",
            ));
        }
        let value = source_output_invocation_use_v1(
            index,
            &captured,
            (1, segment.source_block.index(), 0, 4, 1, 0, 0),
            claim.source_local,
            budget,
        )?;
        let source = source_output_invocation_source_anchor_v1(
            view,
            index,
            claim.source_local,
            value,
            budget,
        )?;
        let (original, original_root) =
            source_output_invocation_n_root_v1(view, index, source, budget)?;
        let assertion = view
            .source
            .assert_origins()
            .assert_condition(
                candidate.selected_root,
                candidate.selected_function,
                segment.source_block,
                budget,
            )
            .map_err(Error::SourceOrigin)?;
        let SemanticKirAssertConditionOutcomeV1::Emitted {
            condition_use,
            definition,
            ..
        } = assertion.outcome()
        else {
            return Err(Error::Invalid("invocation assertion condition was elided"));
        };
        let Def::Result {
            operation,
            result: 0,
        } = definition
        else {
            return Err(Error::Invalid(
                "invocation guard condition is not a direct Compare",
            ));
        };
        budget.charge_work(5).map_err(Error::Resource)?;
        if operation.block.function != index.canonical
            || !matches!(condition_use, Use::TerminatorOperand { block, operand: 0 } if block == operation.block)
        {
            return Err(Error::Invalid("invocation assertion occurrence differs"));
        }
        let compare = body
            .blocks
            .get(operation.block.block as usize)
            .and_then(|block| block.operations.get(operation.operation as usize))
            .ok_or(Error::Invalid("invocation original Compare absent"))?;
        let OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: original_value,
            ..
        } = &compare.kind
        else {
            return Err(Error::Invalid(
                "invocation original guard is not a less-than Compare",
            ));
        };
        source_output_invocation_n_ancestry_v1(
            view,
            index,
            *original_value,
            original_root,
            budget,
        )?;
        let coordinate = Use::OperationOperand {
            operation,
            operand: 0,
        };
        let row = assert_origin_find_v1(
            &view.checked_control_rows.compare_uses,
            budget,
            |row, budget| {
                budget.charge_work(1)?;
                Ok(row.input.coordinate.cmp(&coordinate))
            },
        )
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("invocation checked Compare use absent"))?;
        let row = view.checked_control_rows.compare_uses[row];
        let output =
            source_output_control_literal_row_v1(row, coordinate, *original_value, budget)?;
        budget.charge_work(2).map_err(Error::Resource)?;
        if !matches!(output.coordinate, Use::OperationOperand { operation, .. }
            if operation.block.function == index.canonical)
        {
            return Err(Error::Invalid("invocation mapped Compare function differs"));
        }
        if source_output_control_use_identity_v1(inventory, output.coordinate, budget)? != output {
            return Err(Error::Invalid("invocation mapped Compare use differs"));
        }
        let (output_root, output_value) = source_output_invocation_output_root_v1(
            inventory,
            index.canonical,
            output.value,
            budget,
        )?;
        budget.charge_work(8).map_err(Error::Resource)?;
        let current = (source, original, output_root, output_value);
        if anchor
            .replace(current)
            .is_some_and(|previous| previous != current)
        {
            return Err(Error::Invalid("invocation guard anchors differ"));
        }
        assert_origin_push_v1(
            uses,
            SourceOutputProjectionLiteralUseV1 {
                guard: segment.source_block,
                input: row.input,
                output,
            },
            budget,
        )
        .map_err(Error::SourceOrigin)?;
    }
    let (source, original, output, _) = anchor.ok_or(Error::Invalid(
        "invocation has no retained checked guard use",
    ))?;
    Ok(SourceOutputProjectionArgumentV1 {
        ranked_value: claim.ranked_value,
        source_local: claim.source_local,
        component: claim.component,
        scalar: source_output_address_u64_v1(),
        origin: SourceOutputProjectionLeafOriginV1::Invocation(
            SourceOutputProjectionInvocationV1 {
                source_index,
                source,
                original,
                output,
                symbol,
                first_use,
                end_use: uses.len(),
                identity_getter: false,
            },
        ),
    })
}

struct SourceOutputIdentityStoreV1 {
    site: (u32, u32),
    operand: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    seen: bool,
    address: Option<SourceOutputIdentityStoreAddressV1>,
}

struct SourceOutputIdentityStoreAddressV1 {
    original: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    output: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    pointer: SourceOutputControlUseIdentityV1,
    gep: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    allocation: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceOutputIdentitySourceFactsV1 {
    receiver: SemanticLocalIdV1,
    receiver_value: SsaValueV1,
    receiver_site: (u32, u32),
    slice_entry: SsaValueV1,
    option: SemanticLocalIdV1,
    payload: SemanticLocalIdV1,
    payload_value: SsaValueV1,
    payload_site: (u32, u32),
}

struct SourceOutputIdentityAddressV1 {
    invocation: SourceOutputProjectionInvocationV1,
    source_facts: SourceOutputIdentitySourceFactsV1,
    witness: SemanticLocalIdV1,
    slice: SemanticLocalIdV1,
    original_pointer: ValueId,
    output_slice: ValueId,
    ranked_index: ProductionRankedValueV1,
    ranked_extent: ProductionRankedValueV1,
    stores: Vec<SourceOutputIdentityStoreV1>,
}

struct SourceOutputIdentitySourceV1 {
    anchor: SourceOutputInvocationSourceAnchorV1,
    facts: SourceOutputIdentitySourceFactsV1,
    slice: SemanticLocalIdV1,
    discriminator: SemanticLocalIdV1,
    discriminator_site: (u32, u32),
    switch: SemanticBlockIdV1,
    some: SemanticBlockIdV1,
    none: SemanticBlockIdV1,
    some_ordinal: u32,
    fallback: Option<SemanticBlockIdV1>,
    stores: Vec<SourceOutputIdentityStoreV1>,
    some_region: Vec<bool>,
}

struct SourceOutputIdentityGetterV1 {
    source: SourceOutputIdentitySourceV1,
    invocation: SourceOutputProjectionInvocationV1,
    witness: SemanticLocalIdV1,
    original_compare: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    original_pointer: ValueId,
    output_compare: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    output_condition: ValueId,
    output_index: ValueId,
    output_slice: ValueId,
    output_allocation: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    ranked_index: ProductionRankedValueV1,
    ranked_extent: ProductionRankedValueV1,
    source_argument: u32,
}

fn source_output_identity_address_move_v1(
    identity: SourceOutputIdentityGetterV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputIdentityAddressV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    // Existing transfer15 plus twelve for the retained fixed source facts.
    // No source/Store scan is repeated and the Store allocation is moved.
    budget.charge_work(27).map_err(Error::Resource)?;
    budget
        .reserve_storage(std::mem::size_of::<Option<SourceOutputIdentityAddressV1>>())
        .map_err(Error::Resource)?;
    let released = identity
        .source
        .some_region
        .capacity()
        .checked_mul(std::mem::size_of::<bool>())
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<Option<SourceOutputIdentityGetterV1>>())
        })
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    let SourceOutputIdentityGetterV1 {
        source,
        invocation,
        witness,
        original_pointer,
        output_slice,
        ranked_index,
        ranked_extent,
        ..
    } = identity;
    let SourceOutputIdentitySourceV1 {
        facts,
        slice,
        stores,
        some_region,
        ..
    } = source;
    let address = SourceOutputIdentityAddressV1 {
        invocation,
        source_facts: facts,
        witness,
        slice,
        original_pointer,
        output_slice,
        ranked_index,
        ranked_extent,
        stores,
    };
    drop(some_region);
    budget.release_storage(released).map_err(Error::Resource)?;
    Ok(address)
}

#[cfg(test)]
mod identity_address_transfer_components_v1 {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirBlockCoordinateV1 as Block,
        CanonicalKirFunctionCoordinateV1 as Function,
    };

    const PREFIX: usize = 37;

    // Unauthenticated private data for ownership/accounting tests only. This
    // never constructs a public control, completed analysis or address owner.
    fn descriptor() -> SourceOutputIdentityGetterV1 {
        let operation = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
            block: Block {
                function: Function(2),
                block: 3,
            },
            operation: 4,
        };
        let definition = fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result {
            operation,
            result: 0,
        };
        let anchor = SourceOutputInvocationSourceAnchorV1 {
            get: fe2o3_mir_model::SsaEdgeIdV1::new(SsaBlockIdV1::new(3), 0),
            producer: fe2o3_mir_model::SsaEdgeIdV1::new(SsaBlockIdV1::new(2), 0),
            raw: SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(4)),
            witness: SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(5)),
        };
        let stores = (0..3)
            .map(|statement| SourceOutputIdentityStoreV1 {
                site: (3, statement),
                operand: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::StoreDestination,
                seen: true,
                address: None,
            })
            .collect();
        SourceOutputIdentityGetterV1 {
            source: SourceOutputIdentitySourceV1 {
                anchor,
                facts: SourceOutputIdentitySourceFactsV1 {
                    receiver: SemanticLocalIdV1::from_index(3),
                    receiver_value: SsaValueV1::Definition(
                        fe2o3_mir_model::SsaDefinitionIdV1::new(6),
                    ),
                    receiver_site: (3, 0),
                    slice_entry: SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(7)),
                    option: SemanticLocalIdV1::from_index(4),
                    payload: SemanticLocalIdV1::from_index(8),
                    payload_value: SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(
                        8,
                    )),
                    payload_site: (4, 0),
                },
                slice: SemanticLocalIdV1::from_index(1),
                discriminator: SemanticLocalIdV1::from_index(6),
                discriminator_site: (3, 0),
                switch: SemanticBlockIdV1::from_index(3),
                some: SemanticBlockIdV1::from_index(4),
                none: SemanticBlockIdV1::from_index(5),
                some_ordinal: 1,
                fallback: None,
                stores,
                some_region: vec![false, true, true, false, false],
            },
            invocation: SourceOutputProjectionInvocationV1 {
                source_index: 0,
                source: anchor,
                original: definition,
                output: definition,
                symbol: 7,
                first_use: 0,
                end_use: 0,
                identity_getter: true,
            },
            witness: SemanticLocalIdV1::from_index(2),
            original_compare: operation,
            original_pointer: ValueId(8),
            output_compare: operation,
            output_condition: ValueId(9),
            output_index: ValueId(10),
            output_slice: ValueId(11),
            output_allocation: definition,
            ranked_index: ProductionRankedValueV1::Argument(0),
            ranked_extent: ProductionRankedValueV1::Argument(1),
            source_argument: 0,
        }
    }

    fn charges(value: &SourceOutputIdentityGetterV1) -> (usize, usize, usize) {
        let stores =
            value.source.stores.capacity() * std::mem::size_of::<SourceOutputIdentityStoreV1>();
        let old = std::mem::size_of::<Option<SourceOutputIdentityGetterV1>>()
            + stores
            + value.source.some_region.capacity() * std::mem::size_of::<bool>();
        let new = std::mem::size_of::<Option<SourceOutputIdentityAddressV1>>();
        (old, new, stores)
    }

    #[test]
    fn identity_address_transfer_moves_store_allocation_and_releases_exact_old_owners() {
        let value = descriptor();
        let facts = value.source.facts;
        let (old, new, stores) = charges(&value);
        let pointer = value.source.stores.as_ptr();
        let capacity = value.source.stores.capacity();
        let mut work = Work::new(27);
        let mut budget = AssertOriginBudgetV1::new(&mut work, PREFIX + old + new);
        budget.reserve_storage(PREFIX).unwrap();
        source_output_global_scratch_scope_v1(&mut budget, |budget| {
            budget.reserve_storage(old).unwrap();
            let result = source_output_identity_address_move_v1(value, budget)?;
            assert_eq!(budget.work(), 27);
            assert_eq!(budget.storage(), PREFIX + new + stores);
            assert_eq!(budget.peak_storage(), PREFIX + old + new);
            assert_eq!(result.stores.as_ptr(), pointer);
            assert_eq!(result.stores.capacity(), capacity);
            assert_eq!(result.stores.len(), 3);
            assert_eq!(result.source_facts, facts);
            assert_eq!(result.witness, SemanticLocalIdV1::from_index(2));
            assert_eq!(result.slice, SemanticLocalIdV1::from_index(1));
            assert_eq!(result.original_pointer, ValueId(8));
            assert_eq!(result.output_slice, ValueId(11));
            drop(result);
            budget.release_storage(new + stores).unwrap();
            assert_eq!(budget.storage(), PREFIX);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), PREFIX);
    }

    #[test]
    fn identity_address_transfer_new_header_denial_drops_input_then_restores_floor() {
        let value = descriptor();
        let (old, new, _) = charges(&value);
        let mut work = Work::new(27);
        let mut budget = AssertOriginBudgetV1::new(&mut work, PREFIX + old + new - 1);
        budget.reserve_storage(PREFIX).unwrap();
        let mut completed = false;
        let result: Result<(), ProductionSourceOutputErrorV1> =
            source_output_global_scratch_scope_v1(&mut budget, |budget| {
                budget.reserve_storage(old).unwrap();
                let moved = source_output_identity_address_move_v1(value, budget)?;
                completed = true;
                drop(moved);
                Ok(())
            });
        assert!(!completed);
        assert!(
            matches!(result, Err(ProductionSourceOutputErrorV1::Resource(
            AssertOriginResourceV1::Storage(error)))
            if error.actual() == PREFIX + old + new && error.limit() == PREFIX + old + new - 1)
        );
        assert_eq!(budget.work(), 27);
        assert_eq!(budget.storage(), PREFIX);
        assert_eq!(budget.failed_storage(), Some(PREFIX + old + new));
    }

    #[test]
    fn identity_address_transfer_post_allocation_push_denial_restores_outer_floor() {
        let value = descriptor();
        let (old, new, stores) = charges(&value);
        // Move27; growth of an empty destination vector1; push1 is denied.
        let mut work = Work::new(28);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 1_000_000);
        budget.reserve_storage(PREFIX).unwrap();
        let mut reached_allocation = false;
        let result: Result<(), ProductionSourceOutputErrorV1> =
            source_output_global_scratch_scope_v1(&mut budget, |budget| {
                budget.reserve_storage(old).unwrap();
                let moved = source_output_identity_address_move_v1(value, budget)?;
                assert_eq!(budget.storage(), PREFIX + new + stores);
                let mut destination = Vec::new();
                let result = assert_origin_push_v1(&mut destination, moved, budget);
                assert!(destination.is_empty());
                assert!(destination.capacity() >= 4);
                reached_allocation = true;
                assert_eq!(
                    budget.storage(),
                    PREFIX
                        + new
                        + stores
                        + destination.capacity()
                            * std::mem::size_of::<SourceOutputIdentityAddressV1>()
                );
                // The failed push consumed/dropped its value. The allocated
                // destination is also dropped before scratch-floor restoration.
                drop(destination);
                result.map_err(ProductionSourceOutputErrorV1::SourceOrigin)
            });
        assert!(reached_allocation);
        assert!(
            matches!(result, Err(ProductionSourceOutputErrorV1::SourceOrigin(
            SemanticKirAssertOriginErrorV1::Resource(AssertOriginResourceV1::Work(error))))
            if error.actual() == 29 && error.limit() == 28)
        );
        assert_eq!(budget.work(), 28);
        assert_eq!(budget.storage(), PREFIX);
    }
}

fn source_output_identity_event_v1<'a>(
    index: &SourceOutputInvocationSourceIndexV1,
    captured: &'a fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    key: SourceOutputInvocationEventKeyV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<&'a fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1, ProductionSourceOutputErrorV1>
{
    use ProductionSourceOutputErrorV1 as Error;
    let found = assert_origin_find_v1(&index.events, budget, |row, budget| {
        budget.charge_work(7)?;
        Ok(row.0.cmp(&key))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid("identity source event absent"))?;
    budget.charge_work(5).map_err(Error::Resource)?;
    let event = captured
        .events()
        .get(index.events[found].1)
        .ok_or(Error::Invalid("identity source event ordinal differs"))?;
    if !event.is_reachable()
        || !event.is_promoted()
        || source_output_invocation_event_key_v1(event) != Some(key)
    {
        return Err(Error::Invalid(
            "identity source event is not exact promoted occurrence",
        ));
    }
    Ok(event)
}

fn source_output_identity_defined_v1(
    index: &SourceOutputInvocationSourceIndexV1,
    captured: &fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    site: (u32, u32),
    local: SemanticLocalIdV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SsaValueV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let event =
        source_output_identity_event_v1(index, captured, (0, site.0, site.1, 2, 0, 2, 0), budget)?;
    match (event.event(), event.resolved()) {
        (
            fe2o3_mir_model::SsaEventV1::Define(original),
            Some(SsaResolvedEventV1::Define {
                variable,
                value: value @ SsaValueV1::Definition(_),
            }),
        ) if original == variable && variable.get() == local.index() => Ok(value),
        _ => Err(Error::Invalid("identity source definition differs")),
    }
}

fn source_output_identity_direct_definition_v1(
    index: &SourceOutputInvocationSourceIndexV1,
    value: SsaValueV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputInvocationDefinitionV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(1).map_err(Error::Resource)?;
    if !matches!(value, SsaValueV1::Definition(_)) {
        return Err(Error::Invalid(
            "identity source merge transport is unsupported",
        ));
    }
    let found = assert_origin_find_v1(&index.definitions, budget, |row, budget| {
        budget.charge_work(4)?;
        Ok(row.0.cmp(&value))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid("identity source definition absent"))?;
    Ok(index.definitions[found].1)
}

fn source_output_identity_plain_place_v1(
    operand: &SemanticOperandV1,
) -> Option<&fe2o3_mir_model::semantic_mir_v1::SemanticPlaceV1> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
            if place.projections().is_empty() =>
        {
            Some(place)
        }
        _ => None,
    }
}

fn source_output_identity_source_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    index: &SourceOutputInvocationSourceIndexV1,
    claim: ProductionProjectionArgumentCandidateV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputIdentitySourceV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticBorrowKindV1, SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1,
        SemanticMutabilityV1, SemanticPointerKindV1, SemanticPointerMetadataV1,
        SemanticProjectionKindV1,
    };
    use fe2o3_pliron::{
        ProductionSemanticSsaEventRoleV1 as Role, ProductionSemanticSsaOccurrenceSiteV1 as Site,
        ProductionSemanticSsaOperandRoleV1 as Operand,
    };
    budget.charge_work(4).map_err(Error::Resource)?;
    if index.source != std::ptr::from_ref(view.source).cast() {
        return Err(Error::Invalid("identity captured owner differs"));
    }
    let semantic = view.source.semantic_ssa().source_semantic();
    let function = semantic
        .functions()
        .get(index.function.index() as usize)
        .ok_or(Error::Invalid("identity source function absent"))?;
    let captured = source_output_invocation_capture_v1(view.source, index.function, budget)?;
    let mut getter = None;
    for (block, source) in function.blocks().iter().enumerate() {
        budget.charge_work(4).map_err(Error::Resource)?;
        let SemanticTerminatorKindV1::Call(call) = source.terminator().kind() else {
            continue;
        };
        if let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
                    disjoint_slice,
                    index_witness,
                    element,
                    raw_index,
                },
            ..
        }) = semantic.callables().get(call.callee().index() as usize)
            && getter
                .replace((
                    block,
                    call,
                    *disjoint_slice,
                    *index_witness,
                    *element,
                    *raw_index,
                ))
                .is_some()
        {
            return Err(Error::Invalid(
                "identity getter source occurrence is not unique",
            ));
        }
    }
    let (getter_block, call, slice_type, witness_type, element, raw_type) =
        getter.ok_or(Error::Invalid("identity getter source occurrence absent"))?;
    budget
        .charge_work(
            call.arguments()
                .len()
                .checked_add(16)
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?,
        )
        .map_err(Error::Resource)?;
    let [receiver_operand, witness_operand] = call.arguments() else {
        return Err(Error::Invalid("identity getter arguments differ"));
    };
    let receiver = source_output_identity_plain_place_v1(receiver_operand)
        .ok_or(Error::Invalid("identity receiver is not plain reference"))?;
    let witness = source_output_identity_plain_place_v1(witness_operand)
        .ok_or(Error::Invalid("identity witness is not plain typed value"))?;
    if witness.local() != claim.source_local
        || witness.ty() != witness_type
        || !matches!(
            semantic
                .types()
                .get(raw_type.index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64
            }))
        )
        || !matches!(
            lower_scalar_type(semantic.types(), element).map_err(Error::SourceReplay)?,
            Type::Scalar(_)
        )
    {
        return Err(Error::Invalid(
            "identity getter witness or scalar element type differs",
        ));
    }
    let Some(SemanticTypeShapeV1::Pointer(receiver_type)) = semantic
        .types()
        .get(receiver.ty().index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Err(Error::Invalid("identity receiver reference type absent"));
    };
    if receiver_type.kind() != SemanticPointerKindV1::Reference
        || receiver_type.mutability() != SemanticMutabilityV1::Mutable
        || receiver_type.metadata() != SemanticPointerMetadataV1::None
        || receiver_type.pointee() != slice_type
    {
        return Err(Error::Invalid(
            "identity receiver is not the mutable wrapper reference",
        ));
    }
    let getter_block = u32::try_from(getter_block)
        .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    let witness_value = source_output_invocation_use_v1(
        index,
        &captured,
        (1, getter_block, 0, 3, 1, 0, 0),
        witness.local(),
        budget,
    )?;
    let definition = source_output_identity_direct_definition_v1(index, witness_value, budget)?;
    let (producer, producer_call) = source_output_invocation_call_v1(
        function,
        index,
        &captured,
        witness.local(),
        witness_value,
        definition,
        budget,
    )?;
    budget
        .charge_work(
            producer_call
                .arguments()
                .len()
                .checked_add(5)
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?,
        )
        .map_err(Error::Resource)?;
    if !producer_call.arguments().is_empty()
        || producer_call
            .destination()
            .is_none_or(|to| to.place().ty() != witness_type)
        || !matches!(semantic.callables().get(producer_call.callee().index() as usize),
            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { index_witness, raw_index }, ..
            }) if *index_witness == witness_type && *raw_index == raw_type)
    {
        return Err(Error::Invalid("identity witness producer differs"));
    }
    let receiver_value = source_output_invocation_use_v1(
        index,
        &captured,
        (1, getter_block, 0, 3, 0, 0, 0),
        receiver.local(),
        budget,
    )?;
    let SourceOutputInvocationDefinitionV1::Event(receiver_definition) =
        source_output_identity_direct_definition_v1(index, receiver_value, budget)?
    else {
        return Err(Error::Invalid(
            "identity receiver has no direct Borrow definition",
        ));
    };
    let Site::Statement {
        block: receiver_block,
        statement: receiver_statement,
    } = captured.events()[receiver_definition].site()
    else {
        return Err(Error::Invalid("identity receiver Borrow statement absent"));
    };
    let SemanticStatementKindV1::Assign(borrow) = function.blocks()[receiver_block.get() as usize]
        .statements()[receiver_statement as usize]
        .kind()
    else {
        return Err(Error::Invalid(
            "identity receiver definition is not assignment",
        ));
    };
    let SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Mutable,
        place: slice,
    } = borrow.value().kind()
    else {
        return Err(Error::Invalid(
            "identity receiver definition is not mutable Borrow",
        ));
    };
    budget.charge_work(9).map_err(Error::Resource)?;
    if borrow.destination().local() != receiver.local()
        || !borrow.destination().projections().is_empty()
        || borrow.destination().ty() != receiver.ty()
        || borrow.value().result_type() != receiver.ty()
        || !slice.projections().is_empty()
        || slice.ty() != slice_type
        || !matches!(
            function
                .locals()
                .get(slice.local().index() as usize)
                .map(|local| local.role()),
            Some(SemanticLocalRoleV1::Argument(_))
        )
    {
        return Err(Error::Invalid("identity receiver Borrow formal differs"));
    }
    source_output_invocation_statement_span_v1(
        view,
        index,
        receiver_block.get(),
        receiver_statement,
        budget,
    )?;
    let slice_value = source_output_invocation_use_v1(
        index,
        &captured,
        (0, receiver_block.get(), receiver_statement, 1, 0, 0, 0),
        slice.local(),
        budget,
    )?;
    let plan = view
        .source
        .semantic_ssa()
        .plan_for_function(index.function)
        .ok_or(Error::Invalid("identity source plan absent"))?
        .plan();
    let mut formal = None;
    for row in plan.entry_definitions() {
        budget.charge_work(3).map_err(Error::Resource)?;
        if row.variable().get() == slice.local().index() && formal.replace(row.value()).is_some() {
            return Err(Error::Invalid("identity formal SSA entry duplicated"));
        }
    }
    if formal != Some(slice_value) {
        return Err(Error::Invalid(
            "identity Borrow does not use the source formal definition",
        ));
    }
    let destination = call
        .destination()
        .ok_or(Error::Invalid("identity getter destination absent"))?;
    let option = destination.place().local();
    let mut option_definition = None;
    for (ordinal, row) in captured.edge_definitions().iter().enumerate() {
        budget.charge_work(5).map_err(Error::Resource)?;
        if row.edge().source().get() == getter_block
            && row.variable().get() == option.index()
            && option_definition.replace((ordinal, row.value())).is_some()
        {
            return Err(Error::Invalid("identity Option edge definition duplicated"));
        }
    }
    let (ordinal, Some(option_value @ SsaValueV1::Definition(_))) =
        option_definition.ok_or(Error::Invalid("identity Option edge definition absent"))?
    else {
        return Err(Error::Invalid("identity Option edge value is not direct"));
    };
    let (get, checked_call) = source_output_invocation_call_v1(
        function,
        index,
        &captured,
        option,
        option_value,
        SourceOutputInvocationDefinitionV1::Edge(ordinal),
        budget,
    )?;
    if !std::ptr::eq(checked_call, call) || get.source().get() != getter_block {
        return Err(Error::Invalid(
            "identity getter CallReturn occurrence differs",
        ));
    }
    let mut discriminator = None;
    let mut payload = None;
    for (block, source) in function.blocks().iter().enumerate() {
        budget.charge_work(1).map_err(Error::Resource)?;
        for (statement, row) in source.statements().iter().enumerate() {
            budget.charge_work(6).map_err(Error::Resource)?;
            let SemanticStatementKindV1::Assign(assignment) = row.kind() else {
                continue;
            };
            let site = (
                u32::try_from(block)
                    .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                u32::try_from(statement)
                    .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
            );
            if let SemanticRvalueKindV1::Discriminant(place) = assignment.value().kind()
                && place.local() == option
            {
                if !place.projections().is_empty()
                    || place.ty() != destination.place().ty()
                    || !assignment.destination().projections().is_empty()
                    || discriminator
                        .replace((site, assignment.destination().local()))
                        .is_some()
                {
                    return Err(Error::Invalid("identity Option discriminator is ambiguous"));
                }
            }
            if let SemanticRvalueKindV1::Use(
                SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
            ) = assignment.value().kind()
                && place.local() == option
            {
                if !matches!(place.projections(), [variant, field]
                    if variant.kind() == SemanticProjectionKindV1::Downcast(1)
                    && field.kind() == SemanticProjectionKindV1::Field(0))
                    || !assignment.destination().projections().is_empty()
                    || assignment.destination().ty() != place.ty()
                    || assignment.value().result_type() != place.ty()
                    || payload
                        .replace((site, assignment.destination().local()))
                        .is_some()
                {
                    return Err(Error::Invalid(
                        "identity Option payload is not unique Some.0",
                    ));
                }
            }
        }
    }
    let (discriminator_site, discriminator) =
        discriminator.ok_or(Error::Invalid("identity discriminant absent"))?;
    let (payload_site, payload) = payload.ok_or(Error::Invalid("identity Some payload absent"))?;
    let discriminator_value = source_output_identity_defined_v1(
        index,
        &captured,
        discriminator_site,
        discriminator,
        budget,
    )?;
    let payload_value =
        source_output_identity_defined_v1(index, &captured, payload_site, payload, budget)?;
    if source_output_invocation_use_v1(
        index,
        &captured,
        (0, discriminator_site.0, discriminator_site.1, 1, 0, 0, 0),
        option,
        budget,
    )? != option_value
        || source_output_invocation_use_v1(
            index,
            &captured,
            (0, payload_site.0, payload_site.1, 0, 0, 0, 0),
            option,
            budget,
        )? != option_value
    {
        return Err(Error::Invalid("identity Option own source uses differ"));
    }
    source_output_invocation_statement_span_v1(
        view,
        index,
        payload_site.0,
        payload_site.1,
        budget,
    )?;
    let mut switch = None;
    for (block, source) in function.blocks().iter().enumerate() {
        budget.charge_work(4).map_err(Error::Resource)?;
        let SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } = source.terminator().kind()
        else {
            continue;
        };
        let Some(place) = source_output_identity_plain_place_v1(discriminant) else {
            continue;
        };
        if place.local() != discriminator {
            continue;
        }
        if switch
            .replace((
                block,
                targets,
                matches!(discriminant, SemanticOperandV1::Move(_)),
            ))
            .is_some()
        {
            return Err(Error::Invalid(
                "identity Option switch occurrence duplicated",
            ));
        }
    }
    let (switch, targets, switch_move) =
        switch.ok_or(Error::Invalid("identity Option switch absent"))?;
    let switch =
        u32::try_from(switch).map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    // Switch discriminants have their own source operand role, not CallArgument.
    let mut switch_use = None;
    for event in captured.events() {
        budget.charge_work(4).map_err(Error::Resource)?;
        if event.site()
            == (Site::Terminator {
                block: SsaBlockIdV1::new(switch),
            })
            && event.operand() == Operand::SwitchDiscriminant
            && event.role() == Role::BaseUse
            && matches!(event.resolved(), Some(SsaResolvedEventV1::Use { variable, .. }) if variable.get() == discriminator.index())
        {
            if !event.is_reachable()
                || !event.is_promoted()
                || switch_use.replace(event.resolved()).is_some()
            {
                return Err(Error::Invalid("identity switch source use duplicated"));
            }
        }
    }
    if !matches!(switch_use, Some(Some(SsaResolvedEventV1::Use { value, .. })) if value == discriminator_value)
    {
        return Err(Error::Invalid(
            "identity switch source discriminator differs",
        ));
    }
    budget
        .charge_work(
            targets
                .values()
                .len()
                .checked_add(7)
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?,
        )
        .map_err(Error::Resource)?;
    let (none, some, some_ordinal, fallback) = match targets.values() {
        [zero] if zero.value() == 0 => {
            (zero.edge().target(), targets.otherwise().target(), 1, None)
        }
        [one] if one.value() == 1 => (targets.otherwise().target(), one.edge().target(), 0, None),
        [zero, one] if zero.value() == 0 && one.value() == 1 => (
            zero.edge().target(),
            one.edge().target(),
            1,
            Some(targets.otherwise().target()),
        ),
        _ => {
            return Err(Error::Invalid(
                "identity switch is not exact zero/one Option selection",
            ));
        }
    };
    if none == some || fallback.is_some_and(|block| block == some || block == none) {
        return Err(Error::Invalid(
            "identity Option successors are not distinct",
        ));
    }
    if let Some(fallback) = fallback {
        let block = function
            .blocks()
            .get(fallback.index() as usize)
            .ok_or(Error::Invalid("identity default block absent"))?;
        if !block.statements().is_empty()
            || !matches!(
                block.terminator().kind(),
                SemanticTerminatorKindV1::Unreachable
            )
        {
            return Err(Error::Invalid("identity default is not empty Unreachable"));
        }
    }
    budget.charge_work(12).map_err(Error::Resource)?;
    let facts = SourceOutputIdentitySourceFactsV1 {
        receiver: receiver.local(),
        receiver_value,
        receiver_site: (receiver_block.get(), receiver_statement),
        slice_entry: slice_value,
        option,
        payload,
        payload_value,
        payload_site,
    };
    let mut result = SourceOutputIdentitySourceV1 {
        anchor: SourceOutputInvocationSourceAnchorV1 {
            get,
            producer,
            raw: option_value,
            witness: witness_value,
        },
        facts,
        slice: slice.local(),
        discriminator,
        discriminator_site,
        switch: SemanticBlockIdV1::from_index(switch),
        some,
        none,
        some_ordinal,
        fallback,
        stores: Vec::new(),
        some_region: Vec::new(),
    };
    source_output_identity_some_region_v1(function, index, &mut result, budget)?;
    if !result
        .some_region
        .get(payload_site.0 as usize)
        .copied()
        .unwrap_or(false)
    {
        return Err(Error::Invalid(
            "identity payload definition is outside exact Some region",
        ));
    }
    for (block, source) in function.blocks().iter().enumerate() {
        budget.charge_work(1).map_err(Error::Resource)?;
        for (statement, row) in source.statements().iter().enumerate() {
            budget.charge_work(4).map_err(Error::Resource)?;
            let (destination, operand) = match row.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    (assignment.destination(), Operand::Destination)
                }
                SemanticStatementKindV1::Store(store) => {
                    (store.destination(), Operand::StoreDestination)
                }
                _ => continue,
            };
            if destination.local() != payload || destination.projections().is_empty() {
                continue;
            }
            if !result.some_region[block]
                || !matches!(destination.projections(), [projection]
                if projection.kind() == SemanticProjectionKindV1::Dereference)
                || destination.ty() != element
            {
                return Err(Error::Invalid(
                    "identity own Store is outside Some or has extra projections",
                ));
            }
            assert_origin_push_v1(
                &mut result.stores,
                SourceOutputIdentityStoreV1 {
                    site: (block as u32, statement as u32),
                    operand,
                    seen: false,
                    address: None,
                },
                budget,
            )
            .map_err(Error::SourceOrigin)?;
        }
    }
    if result.stores.is_empty() {
        return Err(Error::Invalid("identity Some has no own source Store"));
    }
    let mut kills = [false; 3];
    for event in captured.events() {
        budget.charge_work(13).map_err(Error::Resource)?;
        let variable = event.event().variable().get();
        if ![
            receiver.local().index(),
            witness.local().index(),
            option.index(),
            payload.index(),
            discriminator.index(),
        ]
        .contains(&variable)
        {
            continue;
        }
        if !event.is_reachable() || !event.is_promoted() {
            return Err(Error::Invalid(
                "identity source anchor has a nonpromoted occurrence",
            ));
        }
        if event.role() == Role::MoveKill {
            let expected = [
                (
                    Site::Terminator {
                        block: SsaBlockIdV1::new(getter_block),
                    },
                    Operand::CallArgument(0),
                    receiver.local(),
                    receiver_value,
                    matches!(receiver_operand, SemanticOperandV1::Move(_)),
                ),
                (
                    Site::Terminator {
                        block: SsaBlockIdV1::new(getter_block),
                    },
                    Operand::CallArgument(1),
                    witness.local(),
                    witness_value,
                    matches!(witness_operand, SemanticOperandV1::Move(_)),
                ),
                (
                    Site::Terminator {
                        block: SsaBlockIdV1::new(switch),
                    },
                    Operand::SwitchDiscriminant,
                    discriminator,
                    discriminator_value,
                    switch_move,
                ),
            ];
            let found = expected
                .iter()
                .position(|row| {
                    row.0 == event.site()
                        && row.1 == event.operand()
                        && row.2.index() == variable
                        && row.4
                })
                .ok_or(Error::Invalid(
                    "identity source anchor has an unrelated Move kill",
                ))?;
            if kills[found]
                || !matches!(event.resolved(), Some(SsaResolvedEventV1::Kill { previous: Some(value), .. }) if value == expected[found].3)
            {
                return Err(Error::Invalid(
                    "identity Move does not consume the exact source value",
                ));
            }
            kills[found] = true;
            continue;
        }
        if variable == payload.index() && event.role() == Role::BaseUse {
            let Site::Statement { block, statement } = event.site() else {
                return Err(Error::Invalid(
                    "identity payload escapes through a terminator",
                ));
            };
            let key = (block.get(), statement);
            let found = assert_origin_find_v1(&result.stores, budget, |row, budget| {
                budget.charge_work(2)?;
                Ok(row.site.cmp(&key))
            })
            .map_err(Error::SourceOrigin)?
            .ok_or(Error::Invalid("identity payload escapes its own Stores"))?;
            let row = &mut result.stores[found];
            if row.seen
                || row.operand != event.operand()
                || !matches!(event.resolved(), Some(SsaResolvedEventV1::Use { value, .. }) if value == payload_value)
            {
                return Err(Error::Invalid("identity Store source BaseUse differs"));
            }
            row.seen = true;
            continue;
        }
        if event.role() == Role::BaseUse {
            let expected = [
                (
                    receiver.local(),
                    Site::Terminator {
                        block: SsaBlockIdV1::new(getter_block),
                    },
                    Operand::CallArgument(0),
                    receiver_value,
                ),
                (
                    witness.local(),
                    Site::Terminator {
                        block: SsaBlockIdV1::new(getter_block),
                    },
                    Operand::CallArgument(1),
                    witness_value,
                ),
                (
                    option,
                    Site::Statement {
                        block: SsaBlockIdV1::new(discriminator_site.0),
                        statement: discriminator_site.1,
                    },
                    Operand::RvaluePlace,
                    option_value,
                ),
                (
                    option,
                    Site::Statement {
                        block: SsaBlockIdV1::new(payload_site.0),
                        statement: payload_site.1,
                    },
                    Operand::RvalueOperand(0),
                    option_value,
                ),
                (
                    discriminator,
                    Site::Terminator {
                        block: SsaBlockIdV1::new(switch),
                    },
                    Operand::SwitchDiscriminant,
                    discriminator_value,
                ),
            ];
            if !expected.iter().any(|row| row.0.index() == variable && row.1 == event.site()
                && row.2 == event.operand()
                && matches!(event.resolved(), Some(SsaResolvedEventV1::Use { value, .. }) if value == row.3))
            { return Err(Error::Invalid("identity source value has an unrelated use")); }
        } else if event.role() == Role::DestinationDefine {
            let expected = [
                (
                    receiver.local(),
                    (receiver_block.get(), receiver_statement),
                    receiver_value,
                ),
                (payload, payload_site, payload_value),
                (discriminator, discriminator_site, discriminator_value),
            ];
            if !expected.iter().any(|row| row.0.index() == variable
                && event.site() == (Site::Statement { block: SsaBlockIdV1::new(row.1.0), statement: row.1.1 })
                && matches!(event.resolved(), Some(SsaResolvedEventV1::Define { value, .. }) if value == row.2))
            { return Err(Error::Invalid("identity source value is redefined")); }
        } else {
            return Err(Error::Invalid(
                "identity source value has unsupported lifetime or projection occurrence",
            ));
        }
    }
    if kills
        != [
            matches!(receiver_operand, SemanticOperandV1::Move(_)),
            matches!(witness_operand, SemanticOperandV1::Move(_)),
            switch_move,
        ]
    {
        return Err(Error::Invalid(
            "identity source Move occurrences are incomplete",
        ));
    }
    for row in &result.stores {
        budget.charge_work(1).map_err(Error::Resource)?;
        if !row.seen {
            return Err(Error::Invalid("identity Store captured occurrence absent"));
        }
    }
    Ok(result)
}

fn source_output_identity_some_region_v1(
    function: &SemanticFunctionDeclV1,
    index: &SourceOutputInvocationSourceIndexV1,
    source: &mut SourceOutputIdentitySourceV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    for _ in function.blocks() {
        assert_origin_push_v1(&mut source.some_region, false, budget)
            .map_err(Error::SourceOrigin)?;
    }
    let mut target = source.some;
    let mut expected = fe2o3_mir_model::SsaEdgeIdV1::new(
        SsaBlockIdV1::new(source.switch.index()),
        source.some_ordinal,
    );
    loop {
        budget.charge_work(9).map_err(Error::Resource)?;
        let found = assert_origin_find_v1(&index.incoming, budget, |row, budget| {
            budget.charge_work(1)?;
            Ok(row.0.0.cmp(&target.index()))
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("identity Some incoming edge absent"))?;
        let multiple = found
            .checked_sub(1)
            .and_then(|i| index.incoming.get(i))
            .is_some_and(|row| row.0.0 == target.index())
            || index
                .incoming
                .get(found + 1)
                .is_some_and(|row| row.0.0 == target.index());
        if multiple {
            if target == source.some {
                return Err(Error::Invalid(
                    "identity Some has multiple source predecessors",
                ));
            }
            break;
        }
        if index.incoming[found].0.1 != expected || target == function.entry() {
            return Err(Error::Invalid("identity Some predecessor differs"));
        }
        let seen = source
            .some_region
            .get_mut(target.index() as usize)
            .ok_or(Error::Invalid("identity Some block absent"))?;
        if *seen {
            return Err(Error::Invalid("identity Some forwarding cycle"));
        }
        *seen = true;
        match function.blocks()[target.index() as usize]
            .terminator()
            .kind()
        {
            SemanticTerminatorKindV1::Return => break,
            SemanticTerminatorKindV1::Goto(edge) => {
                expected = fe2o3_mir_model::SsaEdgeIdV1::new(SsaBlockIdV1::new(target.index()), 0);
                target = edge.target();
            }
            _ => {
                return Err(Error::Invalid(
                    "identity Some region requires direct acyclic continuation",
                ));
            }
        }
    }
    Ok(())
}

fn source_output_identity_span_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    index: &SourceOutputInvocationSourceIndexV1,
    block: u32,
    statement: Option<u32>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<
    (
        fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
        std::ops::Range<usize>,
    ),
    ProductionSourceOutputErrorV1,
> {
    use ProductionSourceOutputErrorV1 as Error;
    let (physical, first, count) = if let Some(statement) = statement {
        let key = (block, statement);
        let found = assert_origin_find_v1(&index.statements, budget, |row, budget| {
            budget.charge_work(2)?;
            Ok(row.0.cmp(&key))
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("identity original statement span absent"))?;
        let span =
            view.source.correspondence.statement_operation_spans()[index.statements[found].1];
        (
            span.kernel_ir_block(),
            span.first_operation_ordinal(),
            span.operation_count(),
        )
    } else {
        let found = assert_origin_find_v1(&index.terminators, budget, |row, budget| {
            budget.charge_work(1)?;
            Ok(row.0.cmp(&block))
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("identity original terminator span absent"))?;
        let span =
            view.source.correspondence.terminator_operation_spans()[index.terminators[found].1];
        (
            span.kernel_ir_block(),
            span.first_operation_ordinal(),
            span.operation_count(),
        )
    };
    let found = assert_origin_find_v1(&index.blocks, budget, |row, budget| {
        budget.charge_work(1)?;
        Ok(row.0.cmp(&physical))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid("identity original span block absent"))?;
    budget.charge_work(3).map_err(Error::Resource)?;
    let end = (first as usize)
        .checked_add(count as usize)
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    Ok((
        fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
            function: index.canonical,
            block: index.blocks[found].1,
        },
        first as usize..end,
    ))
}

fn source_output_identity_normal_return_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
    edge: fe2o3_mir_model::SsaEdgeIdV1,
    original: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(10).map_err(Error::Resource)?;
    let function = &view.source.semantic_ssa().source_semantic().functions()
        [candidate.selected_function.index() as usize];
    let SemanticTerminatorKindV1::Call(call) = function.blocks()[edge.source().get() as usize]
        .terminator()
        .kind()
    else {
        return Err(Error::Invalid(
            "identity normal source boundary is not Call",
        ));
    };
    let destination = call
        .destination()
        .ok_or(Error::Invalid("identity normal source destination absent"))?;
    let ProductionSourceOutputBlockV1::Materialized {
        original: target, ..
    } = view.block(
        candidate.selected_root,
        candidate.selected_function,
        destination.edge().target(),
        budget,
    )?
    else {
        return Err(Error::Invalid(
            "identity normal source continuation was not materialized",
        ));
    };
    let body = view.source.executable().module().functions[original.function.0 as usize]
        .body
        .as_ref()
        .ok_or(Error::Invalid("identity original body absent"))?;
    let target_block = body
        .blocks
        .get(target.block as usize)
        .ok_or(Error::Invalid("identity original normal target absent"))?;
    let Some(Terminator::Branch {
        target: actual,
        arguments,
    }) = &body.blocks[original.block as usize].terminator
    else {
        return Err(Error::Invalid(
            "identity original normal continuation is not Branch",
        ));
    };
    let plan = view
        .source
        .semantic_ssa()
        .plan_for_function(candidate.selected_function)
        .ok_or(Error::Invalid("identity normal source plan absent"))?
        .plan();
    if target.function != original.function
        || *actual != target_block.id
        || !arguments.is_empty()
        || !target_block.parameters.is_empty()
        || plan
            .edge_arguments(edge)
            .is_none_or(|arguments| !arguments.is_empty())
        || plan
            .transport_variables(SsaBlockIdV1::new(destination.edge().target().index()))
            .is_none_or(|variables| !variables.is_empty())
    {
        return Err(Error::Invalid(
            "identity Option components require unsupported merge transport",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn source_output_identity_getter_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
    index: &SourceOutputInvocationSourceIndexV1,
    source_index: usize,
    claim: ProductionProjectionArgumentCandidateV1,
    uses: &mut Vec<SourceOutputProjectionLiteralUseV1>,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<
    (
        SourceOutputProjectionArgumentV1,
        SourceOutputIdentityGetterV1,
    ),
    ProductionSourceOutputErrorV1,
> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::{
        CanonicalKirDefinitionCoordinateV1 as Def, CanonicalKirOperationCoordinateV1 as Op,
        CanonicalKirUseCoordinateV1 as Use,
    };
    budget.charge_work(5).map_err(Error::Resource)?;
    if claim.component != ProductionProjectionArgumentComponentV1::Scalar {
        return Err(Error::Invalid("identity invocation claim is not scalar"));
    }
    let source = source_output_identity_source_v1(view, index, claim, budget)?;
    let symbol = source_output_invocation_symbol_v1(view, candidate, index.canonical, budget)?;
    let mut extent_claim = None;
    for other in &candidate.control.arguments {
        budget.charge_work(4).map_err(Error::Resource)?;
        if other.component == ProductionProjectionArgumentComponentV1::SliceLength
            && other.source_local == source.slice
            && extent_claim.replace(*other).is_some()
        {
            return Err(Error::Invalid("identity extent claim duplicated"));
        }
    }
    let extent_claim = extent_claim.ok_or(Error::Invalid("identity exact extent claim absent"))?;
    let extent = source_output_control_argument_v1(
        view,
        candidate,
        index.canonical,
        extent_claim,
        uses,
        inventory,
        budget,
    )?;
    let SourceOutputProjectionLeafOriginV1::Formal(formal) = extent.origin else {
        return Err(Error::Invalid(
            "identity extent is not an actual source Slice formal",
        ));
    };
    let Def::FunctionArgument { function, argument } = formal.original else {
        return Err(Error::Invalid("identity original Slice parameter absent"));
    };
    if function != index.canonical {
        return Err(Error::Invalid("identity original Slice function differs"));
    }
    let body = view.source.executable().module().functions[index.canonical.0 as usize]
        .body
        .as_ref()
        .ok_or(Error::Invalid("identity original body absent"))?;
    let slice = *body
        .parameters
        .get(argument as usize)
        .ok_or(Error::Invalid("identity original Slice value absent"))?;
    let semantic_function =
        &view.source.semantic_ssa().source_semantic().functions()[index.function.index() as usize];
    let SemanticLocalRoleV1::Argument(source_argument) =
        semantic_function.locals()[source.slice.index() as usize].role()
    else {
        return Err(Error::Invalid("identity allocation is not a source formal"));
    };
    let (producer_block, producer_span) = source_output_identity_span_v1(
        view,
        index,
        source.anchor.producer.source().get(),
        None,
        budget,
    )?;
    source_output_identity_normal_return_v1(
        view,
        candidate,
        source.anchor.producer,
        producer_block,
        budget,
    )?;
    let [producer] = body.blocks[producer_block.block as usize]
        .operations
        .get(producer_span.clone())
        .ok_or(Error::Invalid(
            "identity original producer span outside block",
        ))?
    else {
        return Err(Error::Invalid(
            "identity original producer is not one intrinsic",
        ));
    };
    budget.charge_work(8).map_err(Error::Resource)?;
    if producer.results.len() != 1
        || producer.results[0].ty != Type::INDEX
        || !matches!(&producer.kind, OperationKind::Intrinsic(intrinsic) if *intrinsic == fe2o3_kernel_ir::IntrinsicOperation::global_id_1d())
    {
        return Err(Error::Invalid(
            "identity original producer is not GlobalX INDEX",
        ));
    }
    let original_index = producer.results[0].id;
    let original = Def::Result {
        operation: Op {
            block: producer_block,
            operation: producer_span.start as u32,
        },
        result: 0,
    };
    let (getter_block, getter_span) = source_output_identity_span_v1(
        view,
        index,
        source.anchor.get.source().get(),
        None,
        budget,
    )?;
    source_output_identity_normal_return_v1(
        view,
        candidate,
        source.anchor.get,
        getter_block,
        budget,
    )?;
    let [length, compare, data, gep] = body.blocks[getter_block.block as usize]
        .operations
        .get(getter_span.clone())
        .ok_or(Error::Invalid(
            "identity original getter span outside block",
        ))?
    else {
        return Err(Error::Invalid(
            "identity original getter is not the exact four-operation recipe",
        ));
    };
    budget.charge_work(24).map_err(Error::Resource)?;
    if [length, compare, data, gep]
        .iter()
        .any(|operation| operation.results.len() != 1)
        || length.results[0].ty != Type::INDEX
        || compare.results[0].ty != Type::BOOL
        || !matches!(length.kind, OperationKind::SliceLength { slice: actual } if actual == slice)
        || !matches!(compare.kind, OperationKind::Compare { predicate: ComparePredicate::LessThan, lhs, rhs }
            if lhs == original_index && rhs == length.results[0].id)
        || !matches!(data.kind, OperationKind::SliceData { slice: actual } if actual == slice)
        || !matches!(gep.kind, OperationKind::GetElementPointer { base, offset }
            if base == data.results[0].id && offset == original_index)
        || data.results[0].ty != gep.results[0].ty
        || !matches!(&gep.results[0].ty, Type::Pointer(pointer)
            if matches!(pointer.pointee.as_ref(), Type::Scalar(_))
                && pointer.address_space == fe2o3_kernel_ir::AddressSpace::Global)
    {
        return Err(Error::Invalid("identity original getter components differ"));
    }
    let original_compare = Op {
        block: getter_block,
        operation: getter_span.start as u32 + 1,
    };
    let coordinate = Use::OperationOperand {
        operation: original_compare,
        operand: 0,
    };
    let found = assert_origin_find_v1(
        &view.checked_control_rows.compare_uses,
        budget,
        |row, budget| {
            budget.charge_work(1)?;
            Ok(row.input.coordinate.cmp(&coordinate))
        },
    )
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid("identity checked Compare use absent"))?;
    let mapped = source_output_control_literal_row_v1(
        view.checked_control_rows.compare_uses[found],
        coordinate,
        original_index,
        budget,
    )?;
    if source_output_control_use_identity_v1(inventory, mapped.coordinate, budget)? != mapped {
        return Err(Error::Invalid("identity actual Compare occurrence differs"));
    }
    let Use::OperationOperand {
        operation: output_compare,
        operand: 0,
    } = mapped.coordinate
    else {
        return Err(Error::Invalid("identity output Compare operand differs"));
    };
    let function = inventory
        .functions()
        .get(index.canonical.0 as usize)
        .ok_or(Error::Invalid("identity output function absent"))?;
    let operation =
        source_output_address_operation_v1(inventory, function, output_compare, budget)?;
    budget.charge_work(8).map_err(Error::Resource)?;
    let OperationKind::Compare {
        predicate: ComparePredicate::LessThan,
        lhs,
        rhs,
    } = operation.operation.kind
    else {
        return Err(Error::Invalid(
            "identity output condition is not exact less-than",
        ));
    };
    if operation.operation.results.len() != 1
        || operation.operation.results[0].ty != Type::BOOL
        || lhs != mapped.value
    {
        return Err(Error::Invalid(
            "identity output Compare result or input differs",
        ));
    }
    let output_condition = operation.operation.results[0].id;
    let (output, output_index) =
        source_output_invocation_output_root_v1(inventory, index.canonical, lhs, budget)?;
    if output_index != lhs {
        return Err(Error::Invalid(
            "identity output index is not a direct GlobalX definition",
        ));
    }
    let definition = inventory
        .definition_for_value(index.canonical, rhs, budget)
        .map_err(Error::Inventory)?
        .ok_or(Error::Invalid("identity output extent definition absent"))?;
    let Def::Result {
        operation: length_op,
        result: 0,
    } = definition.coordinate
    else {
        return Err(Error::Invalid(
            "identity output extent is not direct SliceLength",
        ));
    };
    let length = source_output_address_operation_v1(inventory, function, length_op, budget)?;
    budget.charge_work(5).map_err(Error::Resource)?;
    if definition.ty != &Type::INDEX
        || length.operation.results.len() != 1
        || length.operation.results[0].id != rhs
        || !matches!(length.operation.kind, OperationKind::SliceLength { slice } if slice == formal.output_value)
    {
        return Err(Error::Invalid(
            "identity output extent differs from exact source Slice",
        ));
    }
    let first_use = uses.len();
    assert_origin_push_v1(
        uses,
        SourceOutputProjectionLiteralUseV1 {
            guard: source.switch,
            input: view.checked_control_rows.compare_uses[found].input,
            output: mapped,
        },
        budget,
    )
    .map_err(Error::SourceOrigin)?;
    let invocation = SourceOutputProjectionInvocationV1 {
        source_index,
        source: source.anchor,
        original,
        output,
        symbol,
        first_use,
        end_use: uses.len(),
        identity_getter: true,
    };
    let row = SourceOutputProjectionArgumentV1 {
        ranked_value: claim.ranked_value,
        source_local: claim.source_local,
        component: claim.component,
        scalar: source_output_address_u64_v1(),
        origin: SourceOutputProjectionLeafOriginV1::Invocation(invocation),
    };
    Ok((
        row,
        SourceOutputIdentityGetterV1 {
            source,
            invocation,
            witness: claim.source_local,
            original_compare,
            original_pointer: gep.results[0].id,
            output_compare,
            output_condition,
            output_index,
            output_slice: formal.output_value,
            output_allocation: formal.output,
            ranked_index: claim.ranked_value,
            ranked_extent: extent_claim.ranked_value,
            source_argument,
        },
    ))
}

fn source_output_identity_selection_v1(
    terminator: &Terminator,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(ValueId, u32, u32, Option<u32>), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(5).map_err(Error::Resource)?;
    match terminator {
        Terminator::ConditionalBranch {
            condition,
            then_arguments,
            else_arguments,
            ..
        } if then_arguments.is_empty() && else_arguments.is_empty() => Ok((*condition, 0, 1, None)),
        Terminator::Switch {
            selector,
            cases,
            default_arguments,
            ..
        } => {
            budget.charge_work(cases.len()).map_err(Error::Resource)?;
            if !default_arguments.is_empty() || cases.iter().any(|case| !case.arguments.is_empty())
            {
                return Err(Error::Invalid(
                    "identity switch has physical component transport",
                ));
            }
            match cases.as_slice() {
                [zero] if zero.value == 0 => Ok((*selector, 1, 0, None)),
                [one] if one.value == 1 => Ok((*selector, 0, 1, None)),
                [zero, one] if zero.value == 0 && one.value == 1 => Ok((*selector, 1, 0, Some(2))),
                _ => Err(Error::Invalid(
                    "identity actual switch is not exact zero/one",
                )),
            }
        }
        _ => Err(Error::Invalid(
            "identity actual Option selection is unsupported",
        )),
    }
}

// Fixed-size predicate, prepaid by the original/output selector checks. The
// source type is mandatory: representable 0/1 alone is not a type association.
fn source_output_identity_zero_extension_v1(
    operation: &fe2o3_kernel_ir::Operation,
    expected: &Type,
    input: ValueId,
    output: ValueId,
) -> bool {
    operation.results.len() == 1
        && operation.results[0].id == output
        && &operation.results[0].ty == expected
        && matches!(
            expected,
            Type::Scalar(
                ScalarType::I8
                    | ScalarType::I16
                    | ScalarType::I32
                    | ScalarType::I64
                    | ScalarType::U8
                    | ScalarType::U16
                    | ScalarType::U32
                    | ScalarType::U64
            )
        )
        && matches!(&operation.kind, OperationKind::Cast { kind: CastKind::ZeroExtend, value, to }
            if *value == input && to == expected)
}

#[allow(clippy::too_many_arguments)]
fn source_output_identity_edges_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
    segments: &[SourceOutputProjectionSegmentV1],
    segment: SourceOutputProjectionSegmentV1,
    identity: &SourceOutputIdentityGetterV1,
    index: &SourceOutputInvocationSourceIndexV1,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1 as Placement;
    use fe2o3_kernel_ir::{
        CanonicalKirDefinitionCoordinateV1 as Def, CanonicalKirEdgeCoordinateV1 as Edge,
        CanonicalKirOperationCoordinateV1 as Op, CanonicalKirUseCoordinateV1 as Use,
    };
    budget.charge_work(8).map_err(Error::Resource)?;
    let source = &identity.source;
    let tail = &candidate.lowering.kernel().blocks()[segment.claim.tail as usize];
    let ProductionRankedTerminatorV1::IndexLessThan {
        lhs,
        rhs,
        true_block,
        false_block,
    } = tail.terminator()
    else {
        return Err(Error::Invalid(
            "identity ranked Some condition is not the exact less-than",
        ));
    };
    if *lhs != identity.ranked_index
        || *rhs != identity.ranked_extent
        || segment.claim.end != segment.claim.tail + 1
    {
        return Err(Error::Invalid(
            "identity ranked Some condition operands differ",
        ));
    }
    let before = view.source.executable().module().functions[index.canonical.0 as usize]
        .body
        .as_ref()
        .ok_or(Error::Invalid("identity original control body absent"))?;
    let after = view.output().module().functions[index.canonical.0 as usize]
        .body
        .as_ref()
        .ok_or(Error::Invalid("identity output control body absent"))?;
    let (selector, input_some, input_none, input_default) = source_output_identity_selection_v1(
        before.blocks[segment.original.block as usize]
            .terminator
            .as_ref()
            .ok_or(Error::Invalid("identity original selector absent"))?,
        budget,
    )?;
    let output_block = segment.placement.output;
    let (output_selector, output_some, output_none, output_default) =
        source_output_identity_selection_v1(
            after.blocks[output_block.block as usize]
                .terminator
                .as_ref()
                .ok_or(Error::Invalid("identity output selector absent"))?,
            budget,
        )?;
    if source.fallback.is_some() != input_default.is_some() {
        return Err(Error::Invalid(
            "identity source default occurrence differs from N",
        ));
    }
    let (discriminator_block, discriminator_span) = source_output_identity_span_v1(
        view,
        index,
        source.discriminator_site.0,
        Some(source.discriminator_site.1),
        budget,
    )?;
    let operations = before.blocks[discriminator_block.block as usize]
        .operations
        .get(discriminator_span.clone())
        .ok_or(Error::Invalid("identity original discriminant span absent"))?;
    let present = before.blocks[identity.original_compare.block.block as usize].operations
        [identity.original_compare.operation as usize]
        .results[0]
        .id;
    let semantic = view.source.semantic_ssa().source_semantic();
    let function = &semantic.functions()[candidate.selected_function.index() as usize];
    let discriminator_type = lower_scalar_type(
        semantic.types(),
        function.locals()[source.discriminator.index() as usize].ty(),
    )
    .map_err(Error::SourceReplay)?;
    budget.charge_work(10).map_err(Error::Resource)?;
    let input_definition = match operations {
        [] if discriminator_type == Type::BOOL && selector == present => Def::Result {
            operation: identity.original_compare,
            result: 0,
        },
        [operation]
            if source_output_identity_zero_extension_v1(
                operation,
                &discriminator_type,
                present,
                selector,
            ) =>
        {
            Def::Result {
                operation: Op {
                    block: discriminator_block,
                    operation: discriminator_span.start as u32,
                },
                result: 0,
            }
        }
        _ => {
            return Err(Error::Invalid(
                "identity original discriminant is not the getter BOOL transport",
            ));
        }
    };
    let use_coordinate = Use::TerminatorOperand {
        block: segment.original,
        operand: 0,
    };
    let found = assert_origin_find_v1(&view.checked_control_rows.uses, budget, |row, budget| {
        budget.charge_work(1)?;
        Ok(row.input.coordinate.cmp(&use_coordinate))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid("identity checked selector use absent"))?;
    let use_row = view.checked_control_rows.uses[found];
    let mapped = use_row
        .output
        .ok_or(Error::Invalid("identity checked selector use omitted"))?;
    budget.charge_work(8).map_err(Error::Resource)?;
    if use_row.input.value != selector
        || use_row.input.definition != input_definition
        || mapped.coordinate
            != (Use::TerminatorOperand {
                block: output_block,
                operand: 0,
            })
        || mapped.value != output_selector
        || source_output_control_use_identity_v1(inventory, mapped.coordinate, budget)? != mapped
    {
        return Err(Error::Invalid("identity selector own occurrence differs"));
    }
    let function = inventory
        .functions()
        .get(index.canonical.0 as usize)
        .ok_or(Error::Invalid("identity output control inventory absent"))?;
    let definition = inventory
        .definition_for_value(index.canonical, output_selector, budget)
        .map_err(Error::Inventory)?
        .ok_or(Error::Invalid("identity output selector definition absent"))?;
    budget.charge_work(4).map_err(Error::Resource)?;
    if output_selector == identity.output_condition {
        if definition.ty != &Type::BOOL
            || definition.coordinate
                != (Def::Result {
                    operation: identity.output_compare,
                    result: 0,
                })
        {
            return Err(Error::Invalid("identity output Boolean selector differs"));
        }
    } else {
        let Def::Result {
            operation,
            result: 0,
        } = definition.coordinate
        else {
            return Err(Error::Invalid(
                "identity output selector requires unsupported merge transport",
            ));
        };
        let operation = source_output_address_operation_v1(inventory, function, operation, budget)?;
        budget.charge_work(7).map_err(Error::Resource)?;
        if definition.ty != &discriminator_type
            || !source_output_identity_zero_extension_v1(
                operation.operation,
                &discriminator_type,
                identity.output_condition,
                output_selector,
            )
        {
            return Err(Error::Invalid(
                "identity output selector is not exact BOOL zero extension",
            ));
        }
    }
    for (input_ordinal, output_ordinal, target, ranked) in [
        (input_some, output_some, source.some, *true_block),
        (input_none, output_none, source.none, *false_block),
    ] {
        let edge = source_output_control_edge_v1(
            &view.checked_control_rows,
            Edge {
                source: segment.original,
                successor: input_ordinal,
            },
            budget,
        )?;
        let ProductionSourceOutputBlockV1::Materialized {
            original: expected, ..
        } = view.block(
            candidate.selected_root,
            candidate.selected_function,
            target,
            budget,
        )?
        else {
            return Err(Error::Invalid(
                "identity source successor is not materialized",
            ));
        };
        budget.charge_work(6).map_err(Error::Resource)?;
        if edge.input.target != expected
            || !edge.checked.executable
            || !matches!(edge.checked.placement, Placement::Retained(_))
            || edge.output.as_ref().is_none_or(|actual| {
                actual.coordinate
                    != (Edge {
                        source: output_block,
                        successor: output_ordinal,
                    })
            })
        {
            return Err(Error::Invalid(
                "identity Some/None edge polarity or occurrence differs",
            ));
        }
        source_output_control_argument_rows_v1(&view.checked_control_rows, edge, budget)?;
        source_output_control_target_v1(view, candidate, segments, edge, ranked, budget)?;
    }
    if let Some(input_default) = input_default {
        let fallback = source
            .fallback
            .ok_or(Error::Invalid("identity source default absent"))?;
        let ProductionSourceOutputBlockV1::Materialized { original, .. } = view.block(
            candidate.selected_root,
            candidate.selected_function,
            fallback,
            budget,
        )?
        else {
            return Err(Error::Invalid(
                "identity original default is not materialized",
            ));
        };
        let edge = source_output_control_edge_v1(
            &view.checked_control_rows,
            Edge {
                source: segment.original,
                successor: input_default,
            },
            budget,
        )?;
        source_output_control_argument_rows_v1(&view.checked_control_rows, edge, budget)?;
        budget.charge_work(8).map_err(Error::Resource)?;
        let block = &before.blocks[original.block as usize];
        if original.function != index.canonical
            || edge.input.target != original
            || !block.operations.is_empty()
            || !block.parameters.is_empty()
            || !matches!(block.terminator, Some(Terminator::Unreachable))
        {
            return Err(Error::Invalid(
                "identity original default is not exact empty Unreachable",
            ));
        }
        match (output_default, &edge.output, edge.checked.placement) {
            (Some(ordinal), Some(actual), Placement::Retained(_))
                if actual.coordinate
                    == (Edge {
                        source: output_block,
                        successor: ordinal,
                    }) =>
            {
                let block = after
                    .blocks
                    .get(actual.target.block as usize)
                    .ok_or(Error::Invalid("identity output default absent"))?;
                if actual.target.function != index.canonical
                    || !block.operations.is_empty()
                    || !block.parameters.is_empty()
                    || !matches!(block.terminator, Some(Terminator::Unreachable))
                {
                    return Err(Error::Invalid(
                        "identity output default has effects or changed terminal",
                    ));
                }
            }
            (None, None, Placement::Omitted) if !edge.checked.executable => {}
            _ => return Err(Error::Invalid("identity default disposition differs")),
        }
    } else if output_default.is_some() {
        return Err(Error::Invalid("identity output added an unbound default"));
    }
    Ok(())
}

fn source_output_identity_stores_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
    identity: &mut SourceOutputIdentityGetterV1,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Def;
    budget.charge_work(5).map_err(Error::Resource)?;
    if candidate.access_sources.len() != identity.source.stores.len() {
        return Err(Error::Invalid(
            "identity own Store roster differs from memory projection",
        ));
    }
    let function = inventory
        .functions()
        .get(identity.original_compare.block.function.0 as usize)
        .ok_or(Error::Invalid("identity Store output function absent"))?;
    let before = view.source.executable().module().functions[function.coordinate.0 as usize]
        .body
        .as_ref()
        .ok_or(Error::Invalid("identity Store original body absent"))?;
    for access in candidate.access_sources {
        budget.charge_work(10).map_err(Error::Resource)?;
        let statement = access
            .semantic_statement
            .ok_or(Error::Invalid("identity Store is a terminator access"))?;
        let key = (access.semantic_block, statement);
        let found = assert_origin_find_v1(&identity.source.stores, budget, |row, budget| {
            budget.charge_work(2)?;
            Ok(row.site.cmp(&key))
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid(
            "identity projected Store has no exact source occurrence",
        ))?;
        if !identity.source.stores[found].seen || access.semantic_access_ordinal != 0 {
            return Err(Error::Invalid("identity projected Store ordinal differs"));
        }
        let operation = candidate
            .lowering
            .kernel()
            .blocks()
            .get(access.ranked_block as usize)
            .and_then(|block| block.operations().get(access.ranked_operation as usize))
            .ok_or(Error::Invalid("identity projected Store operation absent"))?;
        if !matches!(operation, ProductionRankedOperationV1::Access { kind: dialect_kernel::AccessKindAttr::Write, indices, .. }
            if indices.as_slice() == [identity.ranked_index])
        {
            return Err(Error::Invalid("identity projected Store index differs"));
        }
        let ProductionSourceOutputGlobalAccessV1::Retained {
            original,
            operation,
            source_argument,
            pointer,
            value: Some(_),
            result: None,
            executable: true,
            ..
        } = view.global_access(
            candidate.selected_root,
            candidate.selected_function,
            access.semantic_block,
            Some(statement),
            0,
            budget,
        )?
        else {
            return Err(Error::Invalid(
                "identity own Store is not an exact retained Global effect",
            ));
        };
        if original.block.function != function.coordinate
            || operation.block.function != function.coordinate
            || source_argument != identity.source_argument
        {
            return Err(Error::Invalid(
                "identity Store source allocation or function differs",
            ));
        }
        let original_store = before
            .blocks
            .get(original.block.block as usize)
            .and_then(|block| block.operations.get(original.operation as usize))
            .ok_or(Error::Invalid("identity original Store absent"))?;
        if !matches!(original_store.kind, OperationKind::Store { pointer, .. } if pointer == identity.original_pointer)
        {
            return Err(Error::Invalid(
                "identity original Store does not use its getter pointer",
            ));
        }
        let actual = source_output_control_use_identity_v1(inventory, pointer.coordinate, budget)?;
        if actual.definition != pointer.definition {
            return Err(Error::Invalid(
                "identity output Store pointer occurrence differs",
            ));
        }
        let Def::Result {
            operation: gep_op,
            result: 0,
        } = actual.definition
        else {
            return Err(Error::Invalid(
                "identity output pointer requires unsupported merge transport",
            ));
        };
        let gep = source_output_address_operation_v1(inventory, function, gep_op, budget)?;
        budget.charge_work(8).map_err(Error::Resource)?;
        let OperationKind::GetElementPointer { base, offset } = gep.operation.kind else {
            return Err(Error::Invalid(
                "identity output Store does not use direct getter GEP",
            ));
        };
        if offset != identity.output_index
            || gep.operation.results.len() != 1
            || gep.operation.results[0].id != actual.value
            || !matches!(&gep.operation.results[0].ty, Type::Pointer(pointer)
                if pointer.address_space == fe2o3_kernel_ir::AddressSpace::Global
                    && matches!(pointer.pointee.as_ref(), Type::Scalar(_)))
        {
            return Err(Error::Invalid(
                "identity output GEP index or pointer type differs",
            ));
        }
        let definition = inventory
            .definition_for_value(function.coordinate, base, budget)
            .map_err(Error::Inventory)?
            .ok_or(Error::Invalid(
                "identity output SliceData definition absent",
            ))?;
        let Def::Result {
            operation: data_op,
            result: 0,
        } = definition.coordinate
        else {
            return Err(Error::Invalid(
                "identity output SliceData requires unsupported merge transport",
            ));
        };
        let data = source_output_address_operation_v1(inventory, function, data_op, budget)?;
        budget.charge_work(5).map_err(Error::Resource)?;
        if definition.ty != &gep.operation.results[0].ty
            || data.operation.results.len() != 1
            || data.operation.results[0].id != base
            || !matches!(data.operation.kind, OperationKind::SliceData { slice } if slice == identity.output_slice)
        {
            return Err(Error::Invalid(
                "identity output Store uses a different Slice allocation",
            ));
        }
        budget.charge_work(12).map_err(Error::Resource)?;
        if identity.source.stores[found]
            .address
            .replace(SourceOutputIdentityStoreAddressV1 {
                original,
                output: operation,
                pointer: actual,
                gep: gep_op,
                allocation: identity.output_allocation,
            })
            .is_some()
        {
            return Err(Error::Invalid("identity own Store address seal duplicated"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod identity_getter_discriminator_components_v1 {
    use super::*;

    fn cast(kind: CastKind, ty: Type) -> fe2o3_kernel_ir::Operation {
        fe2o3_kernel_ir::Operation::effect_free(
            fe2o3_kernel_ir::ValueDef::new(ValueId(9), ty.clone()),
            OperationKind::Cast {
                kind,
                value: ValueId(7),
                to: ty,
            },
        )
    }

    #[test]
    fn exact_boolean_discriminator_transport_is_typed_not_generic_integer_casting() {
        for scalar in [
            ScalarType::I8,
            ScalarType::I16,
            ScalarType::I32,
            ScalarType::I64,
            ScalarType::U8,
            ScalarType::U16,
            ScalarType::U32,
            ScalarType::U64,
        ] {
            let ty = Type::Scalar(scalar);
            let original = cast(CastKind::ZeroExtend, ty.clone());
            assert!(source_output_identity_zero_extension_v1(
                &original,
                &ty,
                ValueId(7),
                ValueId(9)
            ));
            for kind in [CastKind::SignExtend, CastKind::Truncate, CastKind::Bitcast] {
                assert!(!source_output_identity_zero_extension_v1(
                    &cast(kind, ty.clone()),
                    &ty,
                    ValueId(7),
                    ValueId(9)
                ));
            }
            assert!(!source_output_identity_zero_extension_v1(
                &original,
                &ty,
                ValueId(8),
                ValueId(9)
            ));
            assert!(!source_output_identity_zero_extension_v1(
                &original,
                &ty,
                ValueId(7),
                ValueId(10)
            ));
            let other = if scalar == ScalarType::U64 {
                Type::Scalar(ScalarType::I64)
            } else {
                Type::Scalar(ScalarType::U64)
            };
            assert!(!source_output_identity_zero_extension_v1(
                &original,
                &other,
                ValueId(7),
                ValueId(9)
            ));
            let mut changed = original.clone();
            changed.results[0].ty = other;
            assert!(!source_output_identity_zero_extension_v1(
                &changed,
                &ty,
                ValueId(7),
                ValueId(9)
            ));
            changed = original.clone();
            changed.results.push(changed.results[0].clone());
            assert!(!source_output_identity_zero_extension_v1(
                &changed,
                &ty,
                ValueId(7),
                ValueId(9)
            ));
        }
        for ty in [
            Type::BOOL,
            Type::INDEX,
            Type::Scalar(ScalarType::F32),
            Type::Scalar(ScalarType::F64),
        ] {
            assert!(!source_output_identity_zero_extension_v1(
                &cast(CastKind::ZeroExtend, ty.clone()),
                &ty,
                ValueId(7),
                ValueId(9)
            ));
        }
    }

    #[test]
    fn zero_one_selection_preserves_occurrence_polarity_and_rejects_unknown_values() {
        use fe2o3_kernel_ir::{
            CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrWorkBudgetV1 as Work, SwitchCase,
        };
        let branch = |values: &[u64]| Terminator::Switch {
            selector: ValueId(7),
            cases: values
                .iter()
                .enumerate()
                .map(|(ordinal, value)| SwitchCase {
                    value: *value,
                    target: BlockId(ordinal as u32 + 10),
                    arguments: vec![],
                })
                .collect(),
            default_target: BlockId(20),
            default_arguments: vec![],
        };
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 0);
        assert_eq!(
            source_output_identity_selection_v1(&branch(&[0, 1]), &mut budget).unwrap(),
            (ValueId(7), 1, 0, Some(2))
        );
        assert_eq!(
            source_output_identity_selection_v1(&branch(&[1]), &mut budget).unwrap(),
            (ValueId(7), 0, 1, None)
        );
        assert_eq!(
            source_output_identity_selection_v1(&branch(&[0]), &mut budget).unwrap(),
            (ValueId(7), 1, 0, None)
        );
        for values in [&[1, 0][..], &[0, 2], &[2], &[]] {
            assert!(source_output_identity_selection_v1(&branch(values), &mut budget).is_err());
        }
        let mut transported = branch(&[0, 1]);
        if let Terminator::Switch {
            default_arguments, ..
        } = &mut transported
        {
            default_arguments.push(ValueId(8));
        }
        assert!(source_output_identity_selection_v1(&transported, &mut budget).is_err());
        assert_eq!(budget.storage(), 0);
    }
}
