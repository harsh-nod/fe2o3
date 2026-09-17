use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirFunctionRefV1, CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1,
    CanonicalKirOperationRefV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as SliceBudget,
    CanonicalKirAccessCoordinateV1 as SliceAccess,
    CanonicalKirDefinitionCoordinateV1 as SliceDefinition,
    CanonicalKirOperationCoordinateV1 as SliceOperation, KirLocalMemoryEffectRefV1,
};

type SliceResult<T> = Result<T, ProductionSemanticKirErrorV1>;

include!("production_borrowed_call_effects_v1.rs");

/// Inert source access and controlling assertion locators, not an admitted view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSliceAccessSiteV1 {
    root: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    statement: Option<u32>,
    access: u32,
    assertion: SemanticBlockIdV1,
}

/// An inert access selector qualified by an exact inventory call path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSliceAccessPathV1<'a> {
    site: ProductionSliceAccessSiteV1,
    call_path: &'a [usize],
}

impl From<ProductionSliceAccessSiteV1> for ProductionSliceAccessPathV1<'_> {
    fn from(site: ProductionSliceAccessSiteV1) -> Self {
        Self {
            site,
            call_path: &[],
        }
    }
}

impl ProductionSliceAccessSiteV1 {
    /// Selects one original source access, with the existing non-private access census.
    pub const fn new(
        root: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        access: u32,
        assertion: SemanticBlockIdV1,
    ) -> Self {
        Self {
            root,
            function,
            block,
            statement,
            access,
            assertion,
        }
    }

    /// Selects an exact root-to-helper path in the same inventory's call roster.
    /// The indices are inert locators; every actual/formal binding is rechecked.
    pub const fn with_call_path(self, call_path: &[usize]) -> ProductionSliceAccessPathV1<'_> {
        ProductionSliceAccessPathV1 {
            site: self,
            call_path,
        }
    }

    fn unsupported(self, detail: &'static str) -> ProductionSemanticKirErrorV1 {
        ProductionSemanticKirErrorV1::Unsupported {
            function: self.function.index(),
            block: Some(self.block.index()),
            statement: self.statement,
            detail,
        }
    }
}

struct SliceFacts<'a> {
    access: SliceAccess,
    data_carrier: SliceDefinition,
    length_carrier: SliceDefinition,
    input: SliceDefinition,
    index: SliceDefinition,
    memory: MemoryAccess,
    loaded_type: &'a Type,
}

/// Scoped correspondence between one actual read, its slice extent and entry input.
/// This is not allocation ownership, a helper-effect summary, proof or launch authority.
/// The loaded type retains the access width without imposing a target pointer width.
pub struct ProductionSliceAccessViewV1<'a> {
    facts: &'a SliceFacts<'a>,
    source: ProductionArgumentNodeV1<'a>,
    root_input: SliceDefinition,
    call_path: &'a [usize],
}

impl ProductionSliceAccessViewV1<'_> {
    /// Exact root source argument/component selected by checked correspondence.
    pub fn source(&self) -> &ProductionArgumentNodeV1<'_> {
        &self.source
    }
    /// Exact owner-local physical read occurrence.
    pub const fn access(&self) -> SliceAccess {
        self.facts.access
    }
    /// Actual slice definition consumed by SliceData at the read.
    pub const fn data_carrier(&self) -> SliceDefinition {
        self.facts.data_carrier
    }
    /// Actual slice definition consumed by SliceLength at the assertion.
    pub const fn length_carrier(&self) -> SliceDefinition {
        self.facts.length_carrier
    }
    /// Exact function-local whole input carrier, not an allocation alias class.
    pub const fn input(&self) -> SliceDefinition {
        self.facts.input
    }
    /// Exact root input reached through the selected actual/formal call path.
    /// Equal lengths do not equate different inputs or establish disjointness.
    pub const fn root_input(&self) -> SliceDefinition {
        self.root_input
    }
    /// Exact inventory call indices, in root-to-access order; empty for root reads.
    pub fn call_path(&self) -> &[usize] {
        self.call_path
    }
    /// Common index definition after only whole-value transport and lossless index bitcasts.
    pub const fn index(&self) -> SliceDefinition {
        self.facts.index
    }
    /// Actual address-space, alignment and volatility contract.
    pub const fn memory(&self) -> MemoryAccess {
        self.facts.memory
    }
    /// Exact typed result of the verified load.
    pub fn loaded_type(&self) -> &Type {
        self.facts.loaded_type
    }
}

impl ProductionPreRankedKirOwnerV1 {
    /// Joins a source read to its live slice carrier, extent and successful assertion edge.
    ///
    /// The caller retains the executable, assertion-origin and inventory storage receipts.
    /// Queries share the existing phase ledger and restore temporary storage on Result return.
    /// Reads require the unique emitted assertion-success predecessor. Helper reads
    /// also require an explicit call path and exact shared-slice actual/formal transport.
    /// Their unchanged direct source descriptors require a retained source/SSA
    /// occurrence capture, reserved by the caller alongside the graph receipts.
    /// Helper indices require exact u64 constant/copy assignment recipes; other
    /// source index origins remain unsupported.
    /// Projected aggregate captures await their separate source/storage relation.
    /// This query does not discharge other effects or preserve caller facts across writes.
    /// Elided assertions and reconstructed slice views remain explicit unsupported cases.
    /// Copied coordinates are inert; the borrowed view cannot leave the callback.
    ///
    /// ```no_run
    /// use fe2o3_lower_mir_kernel::{ProductionPreRankedKirOwnerV1, ProductionSliceAccessSiteV1,
    ///     ProductionSemanticKirErrorV1};
    /// use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
    /// use fe2o3_kernel_ir::{CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    ///     CanonicalKirAccessCoordinateV1};
    /// fn inspect(owner: &ProductionPreRankedKirOwnerV1, inventory: &CanonicalKirInventoryV1<'_>,
    ///     site: ProductionSliceAccessSiteV1, budget: &mut Budget<'_>)
    ///     -> Result<CanonicalKirAccessCoordinateV1, ProductionSemanticKirErrorV1> {
    ///     owner.with_checked_slice_access_v1(inventory, site, budget, |view| Ok(view.access()))
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::{ProductionPreRankedKirOwnerV1, ProductionSliceAccessSiteV1};
    /// use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape(owner: &ProductionPreRankedKirOwnerV1, inventory: &CanonicalKirInventoryV1<'_>,
    ///     site: ProductionSliceAccessSiteV1, budget: &mut Budget<'_>) {
    ///     let saved = owner.with_checked_slice_access_v1(inventory, site, budget,
    ///         |view| Ok(view)).unwrap();
    ///     let _ = saved.input();
    /// }
    /// ```
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::{ProductionPreRankedKirOwnerV1, ProductionSliceAccessSiteV1};
    /// use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape_source(owner: &ProductionPreRankedKirOwnerV1, inventory: &CanonicalKirInventoryV1<'_>,
    ///     site: ProductionSliceAccessSiteV1, budget: &mut Budget<'_>) {
    ///     let saved = owner.with_checked_slice_access_v1(inventory, site, budget,
    ///         |view| Ok(view.source())).unwrap();
    ///     let _ = saved.source_path();
    /// }
    /// ```
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::{ProductionPreRankedKirOwnerV1, ProductionSliceAccessSiteV1};
    /// use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape_path(owner: &ProductionPreRankedKirOwnerV1, inventory: &CanonicalKirInventoryV1<'_>,
    ///     site: ProductionSliceAccessSiteV1, budget: &mut Budget<'_>) {
    ///     let path = [0];
    ///     let saved = owner.with_checked_slice_access_v1(inventory, site.with_call_path(&path),
    ///         budget, |view| Ok(view.call_path())).unwrap();
    ///     let _ = saved.len();
    /// }
    /// ```
    pub fn with_checked_slice_access_v1<'p, R>(
        &self,
        inventory: &CanonicalKirInventoryV1<'_>,
        site: impl Into<ProductionSliceAccessPathV1<'p>>,
        budget: &mut SliceBudget<'_>,
        use_view: impl for<'s> FnOnce(&ProductionSliceAccessViewV1<'s>) -> SliceResult<R>,
    ) -> SliceResult<R> {
        with_checked_borrowed_slice_access_v1(
            CanonicalCallSubjectV1::owner(self),
            self.assert_origins(),
            inventory,
            site,
            budget,
            use_view,
        )
    }
}

/// Pre-admission composition over the existing sealed source/graph attachments.
/// A successful query covers only its selected read, never the helper's whole body.
pub(super) fn with_checked_borrowed_slice_access_v1<'p, R>(
    subject: CanonicalCallSubjectV1<'_>,
    assertions: SemanticKirAssertOriginsV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    site: impl Into<ProductionSliceAccessPathV1<'p>>,
    budget: &mut SliceBudget<'_>,
    use_view: impl for<'s> FnOnce(&ProductionSliceAccessViewV1<'s>) -> SliceResult<R>,
) -> SliceResult<R> {
    let ProductionSliceAccessPathV1 { site, call_path } = site.into();
    with_canonical_call_scratch_v1(budget, |budget| {
        budget.charge_work(2)?;
        if !std::ptr::eq(assertions.executable(), subject.executable)
            || !std::ptr::eq(assertions.semantic_ssa, subject.semantic_ssa)
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let function = slice_function(subject, inventory, site, !call_path.is_empty(), budget)?;
        let facts = super::value_origin_v1::with_whole_value_origins_v1(
            inventory,
            subject.executable,
            function.coordinate,
            budget,
            |origins, budget| {
                let query = SliceQuery {
                    subject,
                    assertions,
                    inventory,
                    function,
                    site,
                    origins,
                };
                let facts = query.facts(budget)?;
                if !call_path.is_empty() {
                    checked_borrowed_slice_source_read_v1(&query, &facts, budget)?;
                }
                Ok::<_, ProductionSemanticKirErrorV1>(facts)
            },
        )
        .map_err(slice_inventory_error)??;
        let (root_function, root_input) = checked_borrowed_slice_call_input_v1(
            subject, inventory, site, call_path, &facts, budget,
        )?;
        let SliceDefinition::FunctionArgument { argument, .. } = root_input else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        with_owner_arguments_v1(
            subject.semantic_ssa.source_semantic(),
            subject.executable.module(),
            subject.correspondence,
            (site.root, root_function),
            budget,
            |arguments| {
                let mut matches = 0_usize;
                arguments.visit_nodes(|node| {
                    if matches!(node.coverage(), ProductionArgumentCoverageV1::Parameter(value)
                    if value.slot() == argument as usize)
                    {
                        matches += 1;
                    }
                    Ok(())
                })?;
                if matches != 1 {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                let mut use_view = Some(use_view);
                let mut result = None;
                arguments.visit_nodes(|source| {
                    if matches!(source.coverage(), ProductionArgumentCoverageV1::Parameter(value)
                    if value.slot() == argument as usize)
                    {
                        let visit = use_view
                            .take()
                            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                        result = Some(visit(&ProductionSliceAccessViewV1 {
                            facts: &facts,
                            source,
                            root_input,
                            call_path,
                        })?);
                    }
                    Ok(())
                })?;
                result.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            },
        )
    })
}

struct SliceQuery<'a, 's> {
    subject: CanonicalCallSubjectV1<'a>,
    assertions: SemanticKirAssertOriginsV1<'a>,
    inventory: &'a CanonicalKirInventoryV1<'a>,
    function: &'a CanonicalKirFunctionRefV1<'a>,
    site: ProductionSliceAccessSiteV1,
    origins: &'s super::value_origin_v1::WholeValueOriginsV1<'a>,
}

fn slice_inventory_error(error: CanonicalKirInventoryErrorV1) -> ProductionSemanticKirErrorV1 {
    match error {
        CanonicalKirInventoryErrorV1::Resource(error) => error.into(),
        CanonicalKirInventoryErrorV1::InconsistentOwner => {
            ProductionSemanticKirErrorV1::CorrespondenceMismatch
        }
    }
}

fn slice_function<'a>(
    subject: CanonicalCallSubjectV1<'a>,
    inventory: &'a CanonicalKirInventoryV1<'a>,
    site: ProductionSliceAccessSiteV1,
    helper: bool,
    budget: &mut SliceBudget<'_>,
) -> SliceResult<&'a CanonicalKirFunctionRefV1<'a>> {
    budget.charge_work(subject.correspondence.lowered_functions().len())?;
    if !inventory.belongs_to(subject.executable) {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let mut rows = subject
        .correspondence
        .lowered_functions()
        .iter()
        .filter(|row| {
            row.correspondence_owner() == site.root && row.semantic_function() == site.function
        });
    let row = rows
        .next()
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if rows.next().is_some() {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let function = inventory
        .function_for_name(row.kernel_ir_function().as_str(), budget)
        .map_err(slice_inventory_error)?
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    budget.charge_work(6)?;
    let semantic = subject.semantic_ssa.source_semantic();
    let source = semantic
        .functions()
        .get(site.function.index() as usize)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let root = semantic
        .functions()
        .get(site.root.index() as usize)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if root.role() != SemanticFunctionRoleV1::KernelRoot {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    // A correspondence role is only a locator. Authenticate both attached owners
    // before allowing an empty path to select the root-only query.
    let actual_role = match (source.role(), function.function.role) {
        (SemanticFunctionRoleV1::KernelRoot, fe2o3_kernel_ir::FunctionRole::KernelEntry)
            if site.function == site.root =>
        {
            SemanticKirFunctionRoleV1::KernelEntry
        }
        (SemanticFunctionRoleV1::InternalHelper, fe2o3_kernel_ir::FunctionRole::InternalHelper)
            if site.function != site.root =>
        {
            SemanticKirFunctionRoleV1::InternalHelper
        }
        _ => return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
    };
    if row.role() != actual_role {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let expected_role = if helper {
        SemanticKirFunctionRoleV1::InternalHelper
    } else {
        SemanticKirFunctionRoleV1::KernelEntry
    };
    if actual_role != expected_role {
        return Err(site.unsupported("slice access requires an exact root-qualified call path"));
    }
    Ok(function)
}

impl<'a> SliceQuery<'a, '_> {
    fn operation(
        &self,
        coordinate: SliceOperation,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<&'a CanonicalKirOperationRefV1<'a>> {
        budget.charge_work(4)?;
        if coordinate.block.function != self.function.coordinate {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let blocks = self
            .inventory
            .blocks()
            .get(self.function.blocks.clone())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let block = blocks
            .get(coordinate.block.block as usize)
            .filter(|block| block.coordinate == coordinate.block)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        self.inventory
            .operations()
            .get(block.operations.clone())
            .and_then(|operations| operations.get(coordinate.operation as usize))
            .filter(|operation| operation.coordinate == coordinate)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    }

    fn origin(&self, value: ValueId, budget: &mut SliceBudget<'_>) -> SliceResult<SliceDefinition> {
        self.origins
            .resolve(value, budget)
            .map_err(slice_inventory_error)?
            .ok_or_else(|| {
                self.site
                    .unsupported("slice access has conflicting or ungrounded SSA origins")
            })
    }

    fn defining_operation(
        &self,
        value: ValueId,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<&'a CanonicalKirOperationRefV1<'a>> {
        match self.origin(value, budget)? {
            SliceDefinition::Result {
                operation,
                result: 0,
            } => self.operation(operation, budget),
            _ => Err(self
                .site
                .unsupported("slice access operand has no supported defining operation")),
        }
    }

    fn is_retained_private(
        &self,
        mut pointer: ValueId,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<bool> {
        for _ in 0..self.function.definitions.len() {
            budget.charge_work(1)?;
            let origin = self
                .origins
                .resolve(pointer, budget)
                .map_err(slice_inventory_error)?;
            let origin = origin.ok_or_else(|| {
                self.site.unsupported(
                    "retained private storage has conflicting or ungrounded SSA origins",
                )
            })?;
            let SliceDefinition::Result {
                operation,
                result: 0,
            } = origin
            else {
                return Ok(false);
            };
            let actual = self.operation(operation, budget)?;
            budget.charge_work(
                self.subject
                    .correspondence
                    .synthetic_operation_spans()
                    .len(),
            )?;
            let block = &self.inventory.blocks()
                [self.function.blocks.start + operation.block.block as usize];
            let location =
                FunctionOperationLocation::new(block.block.id, operation.operation as usize);
            if retained_local_storage_allocation_v1(
                actual.operation,
                location,
                self.subject.correspondence,
                self.site.root,
                self.site.function,
            ) {
                return Ok(true);
            }
            match &actual.operation.kind {
                OperationKind::Cast {
                    kind: CastKind::RestrictPointerAccess,
                    value,
                    ..
                } => pointer = *value,
                _ => return Ok(false),
            }
        }
        Err(self
            .site
            .unsupported("retained private storage transport is cyclic"))
    }

    fn source_span(&self, budget: &mut SliceBudget<'_>) -> SliceResult<(BlockId, u32, u32)> {
        let rows = self.subject.correspondence;
        let mut selected = None;
        match self.site.statement {
            Some(statement) => {
                budget.charge_work(rows.statement_operation_spans().len())?;
                for row in rows.statement_operation_spans() {
                    if row.correspondence_owner() == self.site.root
                        && row.semantic_function() == self.site.function
                        && row.semantic_block() == self.site.block
                        && row.statement_ordinal() == statement
                        && selected
                            .replace((
                                row.kernel_ir_block(),
                                row.first_operation_ordinal(),
                                row.operation_count(),
                            ))
                            .is_some()
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                }
            }
            None => {
                budget.charge_work(rows.terminator_operation_spans().len())?;
                for row in rows.terminator_operation_spans() {
                    if row.correspondence_owner() == self.site.root
                        && row.semantic_function() == self.site.function
                        && row.semantic_block() == self.site.block
                        && selected
                            .replace((
                                row.kernel_ir_block(),
                                row.first_operation_ordinal(),
                                row.operation_count(),
                            ))
                            .is_some()
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                }
            }
        }
        selected.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    }

    fn access(&self, budget: &mut SliceBudget<'_>) -> SliceResult<SliceAccess> {
        let (block, first, count) = self.source_span(budget)?;
        let block = self
            .inventory
            .block_for_id(self.function.coordinate, block, budget)
            .map_err(slice_inventory_error)?
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let end = first
            .checked_add(count)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let operations = self
            .inventory
            .operations()
            .get(block.operations.clone())
            .and_then(|operations| operations.get(first as usize..end as usize))
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        budget.charge_work(operations.len())?;
        let mut ordinal = 0_u32;
        let mut selected = None;
        for operation in operations {
            let effects = self
                .inventory
                .effects()
                .get(operation.effects.clone())
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            budget.charge_work(effects.len())?;
            let mut effects = effects.iter().filter(|effect| {
                matches!(
                    effect.effect,
                    KirLocalMemoryEffectRefV1::Read(_)
                        | KirLocalMemoryEffectRefV1::Write(_)
                        | KirLocalMemoryEffectRefV1::VolatileRead(_)
                        | KirLocalMemoryEffectRefV1::VolatileWrite(_)
                        | KirLocalMemoryEffectRefV1::Atomic { .. }
                )
            });
            try_visit_kir_memory_accesses_v1(
                operation.operation,
                |(pointer, access, space, atomic)| -> SliceResult<()> {
                    budget.charge_work(1)?;
                    let effect = effects
                        .next()
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                    let plain_private = space == dialect_kernel::MemorySpaceAttr::Private
                        && atomic.is_none()
                        && matches!(
                            (&operation.operation.kind, access),
                            (
                                OperationKind::Load { .. },
                                dialect_kernel::AccessKindAttr::Read
                            ) | (
                                OperationKind::Store { .. },
                                dialect_kernel::AccessKindAttr::Write
                            )
                        );
                    if plain_private && self.is_retained_private(pointer, budget)? {
                        return Ok(());
                    }
                    if ordinal == self.site.access {
                        selected = Some(effect.coordinate);
                    }
                    ordinal = ordinal
                        .checked_add(1)
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                    Ok(())
                },
            )?;
            if effects.next().is_some() {
                return Err(self
                    .site
                    .unsupported("source slice span has an unmodeled memory effect"));
            }
        }
        selected.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    }

    fn index_origin(
        &self,
        mut value: ValueId,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<SliceDefinition> {
        for _ in 0..self.function.definitions.len() {
            budget.charge_work(1)?;
            let definition = self
                .inventory
                .definition_for_value(self.function.coordinate, value, budget)
                .map_err(slice_inventory_error)?
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if !matches!(
                definition.ty,
                Type::Scalar(ScalarType::Index | ScalarType::U64)
            ) {
                return Err(self
                    .site
                    .unsupported("slice bounds require the exact unsigned index domain"));
            }
            let origin = self.origin(value, budget)?;
            if let SliceDefinition::Result {
                operation,
                result: 0,
            } = origin
            {
                let operation = self.operation(operation, budget)?;
                if let OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value: input,
                    to,
                } = &operation.operation.kind
                    && matches!(to, Type::Scalar(ScalarType::Index | ScalarType::U64))
                {
                    value = *input;
                    continue;
                }
            }
            return Ok(origin);
        }
        Err(self.site.unsupported("slice index transport is cyclic"))
    }

    fn facts(&self, budget: &mut SliceBudget<'_>) -> SliceResult<SliceFacts<'a>> {
        let access = self.access(budget)?;
        let read = self.operation(access.operation, budget)?;
        let OperationKind::Load {
            pointer,
            access: memory,
        } = &read.operation.kind
        else {
            return Err(self
                .site
                .unsupported("checked slice correspondence requires a plain read"));
        };
        if memory.address_space != AddressSpace::Global || memory.volatile || access.effect != 0 {
            return Err(self
                .site
                .unsupported("checked slice correspondence requires a nonvolatile global read"));
        }
        let loaded_type = &read
            .operation
            .results
            .first()
            .filter(|_| read.operation.results.len() == 1)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            .ty;
        let address = self.defining_operation(*pointer, budget)?;
        let OperationKind::GetElementPointer { base, offset } = address.operation.kind else {
            return Err(self
                .site
                .unsupported("slice read address is not an exact element projection"));
        };
        let data = self.defining_operation(base, budget)?;
        let OperationKind::SliceData { slice: data_slice } = data.operation.kind else {
            return Err(self
                .site
                .unsupported("slice read address has no exact slice data carrier"));
        };
        let assertion = self.assertions.assert_condition(
            self.site.root,
            self.site.function,
            self.site.assertion,
            budget,
        )?;
        let SemanticKirAssertConditionOutcomeV1::Emitted {
            definition,
            success_edge,
            ..
        } = assertion.outcome()
        else {
            return Err(self
                .site
                .unsupported("slice assertion was elided by an existing rule"));
        };
        if !assertion.expected() || assertion.semantic_success() != self.site.block {
            return Err(self
                .site
                .unsupported("slice read is not the selected positive assertion successor"));
        }
        let edges = self
            .inventory
            .edges()
            .get(self.function.edges.clone())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        budget.charge_work(edges.len())?;
        let mut predecessors = edges
            .iter()
            .filter(|edge| edge.target == access.operation.block);
        if predecessors.next().map(|edge| edge.coordinate) != Some(success_edge)
            || predecessors.next().is_some()
        {
            return Err(self
                .site
                .unsupported("slice read requires the unique assertion-success predecessor"));
        }
        let SliceDefinition::Result {
            operation: compare,
            result: 0,
        } = definition
        else {
            return Err(self
                .site
                .unsupported("slice assertion has no exact comparison definition"));
        };
        let compare = self.operation(compare, budget)?;
        let OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs,
            rhs,
        } = compare.operation.kind
        else {
            return Err(self
                .site
                .unsupported("slice assertion is not an exact less-than comparison"));
        };
        let index = self.index_origin(offset, budget)?;
        if index != self.index_origin(lhs, budget)? {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let SliceDefinition::Result {
            operation: length,
            result: 0,
        } = self.index_origin(rhs, budget)?
        else {
            return Err(self
                .site
                .unsupported("slice assertion does not use a slice length"));
        };
        let length = self.operation(length, budget)?;
        let OperationKind::SliceLength {
            slice: length_slice,
        } = length.operation.kind
        else {
            return Err(self
                .site
                .unsupported("slice assertion does not use a slice length"));
        };
        let input = self.origin(data_slice, budget)?;
        if input != self.origin(length_slice, budget)? {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        if !matches!(input, SliceDefinition::FunctionArgument { .. }) {
            return Err(self
                .site
                .unsupported("slice carrier is not exact whole-entry transport"));
        }
        let carrier = |value, budget: &mut SliceBudget<'_>| -> SliceResult<SliceDefinition> {
            let definition = self
                .inventory
                .definition_for_value(self.function.coordinate, value, budget)
                .map_err(slice_inventory_error)?
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if !matches!(definition.ty, Type::Slice(_)) {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            Ok(definition.coordinate)
        };
        Ok(SliceFacts {
            access,
            data_carrier: carrier(data_slice, budget)?,
            length_carrier: carrier(length_slice, budget)?,
            input,
            index,
            memory: *memory,
            loaded_type,
        })
    }
}
