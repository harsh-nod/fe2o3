/// Same-owner borrowed composition view. It grants no publisher, source-file,
/// execution or artifact authority; all locators are checked against this owner.
pub struct OrderedCompositionInspectionV1<'a> {
    owner: &'a ProductionOrderedCompositionPreRankedKirOwnerV1,
    ledger: usize,
    work_ledger: ArgumentLedgerV1,
    floor: usize,
}
/// Borrowed actual marker/source/canonical association, not an editable graph.
pub struct OrderedCompositionRegionV1<'a> {
    definition: &'a OrderedCompositionSourceDefinitionV1,
    source_function: &'a SemanticFunctionDeclV1,
    source_call: &'a SemanticDirectCallV1,
    site: fe2o3_kernel_ir::OrderedProgramSiteV1,
    operation: &'a Operation,
    program: &'a fe2o3_kernel_ir::Gfx942OrderedProgramV1,
    span: &'a SemanticKirTerminatorOperationSpanV1,
}
impl OrderedCompositionRegionV1<'_> {
    /// Actual retained semantic function declaration.
    pub const fn source_function(&self) -> &SemanticFunctionDeclV1 {
        self.source_function
    }
    /// Actual immutable direct marker call.
    pub const fn source_call(&self) -> &SemanticDirectCallV1 {
        self.source_call
    }
    /// Selected source function coordinate.
    pub const fn semantic_function(&self) -> SemanticFunctionIdV1 {
        self.definition.function
    }
    /// Selected source block coordinate.
    pub const fn semantic_block(&self) -> SemanticBlockIdV1 {
        self.definition.block
    }
    /// Canonical coordinate in this exact composition owner.
    pub const fn canonical_site(&self) -> fe2o3_kernel_ir::OrderedProgramSiteV1 {
        self.site
    }
    /// Actual canonical executable operation.
    pub const fn operation(&self) -> &Operation {
        self.operation
    }
    /// Unchanged descriptors, bindings, input SSA and source identity.
    pub const fn program(&self) -> &fe2o3_kernel_ir::Gfx942OrderedProgramV1 {
        self.program
    }
    /// Normal lowering's actual source-to-canonical terminator interval.
    pub const fn span(&self) -> &SemanticKirTerminatorOperationSpanV1 {
        self.span
    }
}
impl OrderedCompositionInspectionV1<'_> {
    /// Selects one definition by exact inert canonical/semantic identities and
    /// owner-local key. Hash equality is not source authentication.
    pub fn definition(
        &self,
        key: fe2o3_kernel_ir::OrderedProgramDefinitionKeyV1,
        expected_canonical: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV17,
        expected_semantic: &[u8; 32],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<OrderedCompositionRegionV1<'_>, ProductionSemanticKirErrorV1> {
        let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
        budget.charge_work(70)?;
        if self.ledger != budget as *const ArgumentBudgetV1<'_> as usize
            || self.work_ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        if expected_semantic != self.owner.correspondence.semantic_sha256()
            || expected_canonical != self.owner.executable().identity()
        {
            return Err(mismatch());
        }
        let definition = self
            .owner
            .sources
            .definitions
            .get(key.ordinal() as usize)
            .and_then(Option::as_ref)
            .filter(|d| d.key == key)
            .ok_or_else(mismatch)?;
        let canonical = self
            .owner
            .composition()
            .definitions()
            .get(key.ordinal() as usize)
            .filter(|d| d.key() == key)
            .ok_or_else(mismatch)?;
        let semantic = self.owner.semantic_ssa().source_semantic();
        let source_function = semantic
            .functions()
            .get(definition.function.index() as usize)
            .ok_or_else(mismatch)?;
        let block = source_function
            .blocks()
            .get(definition.block.index() as usize)
            .ok_or_else(mismatch)?;
        let SemanticTerminatorKindV1::Call(source_call) = block.terminator().kind() else {
            return Err(mismatch());
        };
        let operation = self
            .owner
            .composition()
            .definition_operation(expected_canonical, key, budget)
            .map_err(ordered_composition_structural_error_v1)?;
        let OperationKind::Gfx942OrderedProgram(program) = &operation.kind else {
            return Err(mismatch());
        };
        budget.charge_work(self.owner.correspondence.terminator_operation_spans.len())?;
        let span = self
            .owner
            .correspondence
            .terminator_operation_spans
            .iter()
            .find(|span| {
                span.correspondence_owner == semantic.roots()[0]
                    && span.semantic_function == definition.function
                    && span.semantic_block == definition.block
                    && span.kernel_ir_block == canonical.site().block()
            })
            .ok_or_else(mismatch)?;
        Ok(OrderedCompositionRegionV1 {
            definition,
            source_function,
            source_call,
            site: canonical.site(),
            operation,
            program,
            span,
        })
    }
}

/// Borrows same-owner definitions without cloning source or executable graphs.
/// The callback is scratch-only and must leave the original cumulative ledger
/// and retained floor unchanged. Actual source publication needs independent
/// backend Instance/span custody; decoded identities never reconstruct it.
pub fn with_ordered_composition_inspection_v1<'w, R>(
    owner: &ProductionOrderedCompositionPreRankedKirOwnerV1,
    budget: &mut ArgumentBudgetV1<'w>,
    consumer: impl for<'s> FnOnce(
        &OrderedCompositionInspectionV1<'s>,
        &mut ArgumentBudgetV1<'w>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    if budget.storage() < owner.live_storage_floor_v1()? {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    with_canonical_call_scratch_v1(budget, |budget| {
        budget.reserve_storage(std::mem::size_of::<OrderedCompositionInspectionV1<'_>>())?;
        let view = OrderedCompositionInspectionV1 {
            owner,
            ledger: budget as *const ArgumentBudgetV1<'_> as usize,
            work_ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
        };
        let result = consumer(&view, budget);
        if view.work_ledger != budget.work_ledger_identity_v1() || budget.storage() != view.floor {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        result
    })
}
