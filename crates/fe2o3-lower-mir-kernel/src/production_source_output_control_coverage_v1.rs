use fe2o3_pliron::ProductionRankedTerminatorV1;

/// Inert source-local anchor for a projection argument. Numeric projection
/// identifiers are not source ABI ordinals. The consumer resolves this local
/// through the replayed importer correspondence before inspecting actual O.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionProjectionArgumentComponentV1 {
    /// A direct scalar formal or independently checked single-definition literal.
    Scalar,
    /// Immutable length metadata of an admitted Slice source formal.
    SliceLength,
}

/// An untrusted projected leaf and the source component it claims to represent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionProjectionArgumentCandidateV1 {
    /// Projection-local Argument or Local(IndexUnknown), not an ABI ordinal.
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
    /// A literal row returns None; use index_leaf for its distinct typed borrow.
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
    pub fn index_leaf(
        &self,
        view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
        candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
        ranked_value: ProductionRankedValueV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Option<ProductionConditionalMemoryIndexLeafV1<'_>>, ProductionSourceOutputErrorV1>
    {
        let ordinal = self.candidate_ordinal_v1(view, candidate, budget)?;
        source_output_control_leaf_rows_v1(
            self.arguments,
            self.literal_uses,
            self.candidates[ordinal].arguments.clone(),
            ranked_value,
            budget,
        )
    }

    fn address_leaf_v1(
        &self,
        view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
        candidate: &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
        ordinal: usize,
        ranked_value: ProductionRankedValueV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Option<ProductionConditionalMemoryIndexLeafV1<'_>>, ProductionSourceOutputErrorV1>
    {
        self.require_candidate_at_v1(view, candidate, ordinal, budget)?;
        source_output_control_leaf_rows_v1(
            self.arguments,
            self.literal_uses,
            self.candidates[ordinal].arguments.clone(),
            ranked_value,
            budget,
        )
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
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(2).map_err(Error::Resource)?;
    match claim.ranked_value {
        ProductionRankedValueV1::Argument(argument)
            if (argument as usize) < candidate.lowering.kernel().argument_count() =>
        {
            Ok(())
        }
        ProductionRankedValueV1::Local(value) => {
            let mut found = false;
            for block in candidate.lowering.kernel().blocks() {
                budget.charge_work(1).map_err(Error::Resource)?;
                for operation in block.operations() {
                    budget.charge_work(2).map_err(Error::Resource)?;
                    if let ProductionRankedOperationV1::IndexUnknown { result } = operation
                        && *result == value
                    {
                        if found {
                            return Err(Error::Invalid("control unknown definition duplicated"));
                        }
                        found = true;
                    }
                }
            }
            // The lowering owner already checked complete SSA definitions and
            // visibility. This check additionally requires the actual unknown
            // opcode, not a candidate assertion about its numerical meaning.
            if found {
                Ok(())
            } else {
                Err(Error::Invalid(
                    "control local anchor is not an actual IndexUnknown",
                ))
            }
        }
        _ => Err(Error::Invalid("control ranked leaf anchor unsupported")),
    }
}

// Wrap the shared normalizer, adding only an independently checked metadata
// symbol for SliceLength(formal). No Call evaluation, memory read, phi guess or
// alternate arithmetic semantics is introduced here.
struct SourceOutputControlNormalizationV1<'a, 'kir, 'ranked, 'ledger, 'limit> {
    inner: SourceOutputScalarNormalizationV1<'a, 'kir, 'ranked, 'ledger, 'limit>,
    literal_uses: &'a [SourceOutputProjectionLiteralUseV1],
    guard: Option<(SemanticBlockIdV1, ValueId)>,
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
                        && *result == value && literal.replace(*bits).is_some()
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
        if matches!(
            source.blocks()[segment.source.index() as usize]
                .terminator()
                .kind(),
            SemanticTerminatorKindV1::Call(_)
        ) && call_counts[segment.source.index() as usize] != 1
        {
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
            && block == access.semantic_block && last >= location
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
                + std::mem::size_of::<ProductionConditionalMemoryControlCoverageV1<'_>>();
            budget.reserve_storage(header).map_err(Error::Resource)?;
            let mut identities = Vec::new();
            let mut arguments = Vec::new();
            let mut literal_uses = Vec::new();
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
                for claim in &candidate.control.arguments {
                    budget.charge_work(2).map_err(Error::Resource)?;
                    source_output_control_ranked_anchor_v1(candidate, *claim, budget)?;
                    let row = source_output_control_argument_v1(
                        self,
                        candidate,
                        canonical,
                        *claim,
                        &mut literal_uses,
                        &inventory,
                        budget,
                    )?;
                    assert_origin_push_v1(&mut arguments, row, budget)
                        .map_err(Error::SourceOrigin)?;
                }
                source_output_ranked_sort_unique_v1(&mut arguments[first..], budget, |a, b| {
                    a.ranked_value.cmp(&b.ranked_value)
                })?;
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
                    let segments = source_output_control_segments_v1(self, candidate, budget)?;
                    source_output_control_calls_and_accesses_v1(
                        self, candidate, canonical, &segments, budget,
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
                        source_output_control_segment_edges_v1(
                            self,
                            candidate,
                            &segments,
                            *segment,
                            &arguments[first..],
                            &mut context,
                        )?;
                    }
                    Ok(())
                })?;
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
                    },
                    budget,
                )
                .map_err(Error::SourceOrigin)?;
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
) -> Result<Option<ProductionConditionalMemoryIndexLeafV1<'a>>, ProductionSourceOutputErrorV1> {
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
            ProductionConditionalMemoryIndexLeafV1::Formal(ProductionConditionalMemoryArgumentV1 {
                row,
                formal,
            })
        }
        SourceOutputProjectionLeafOriginV1::Literal(literal) => {
            ProductionConditionalMemoryIndexLeafV1::Literal(ProductionConditionalMemoryLiteralV1 {
                row,
                literal,
                uses: literal_uses
                    .get(literal.first_use..literal.end_use)
                    .ok_or(Error::Invalid("literal guard-use span absent"))?,
            })
        }
    }))
}
