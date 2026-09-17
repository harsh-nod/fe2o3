// Included beside source/output custody; no detached graph or receipt constructor.

type ScalarOutputResultV1<T> = Result<T, ProductionSourceOutputErrorV1>;
type ScalarOutputOperationV1 = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1;

/// Source-qualified scalar memory occurrence, including checked dead omissions.
#[derive(Debug)]
pub struct ScalarOutputMemoryObligationV1<'view> {
    source: SemanticKirAssertSiteV1,
    statement: Option<u32>,
    access_ordinal: u32,
    original: ScalarOutputOperationV1,
    output: Option<ScalarOutputOperationV1>,
    executable: bool,
    formal: Option<&'view FormalMemoryAccess>,
}

impl ScalarOutputMemoryObligationV1<'_> {
    /// Exact source root, function, block, statement and memory-access ordinal.
    pub const fn source_site(
        &self,
    ) -> (
        SemanticFunctionIdV1,
        SemanticFunctionIdV1,
        SemanticBlockIdV1,
        Option<u32>,
        u32,
    ) {
        (
            self.source.correspondence_owner,
            self.source.semantic_function,
            self.source.semantic_block,
            self.statement,
            self.access_ordinal,
        )
    }
    /// Original N/B occurrence; N/B coordinate custody was checked by the view.
    pub const fn original(&self) -> ScalarOutputOperationV1 {
        self.original
    }
    /// Actual O occurrence, absent only for independently unreachable input.
    pub const fn output(&self) -> Option<ScalarOutputOperationV1> {
        self.output
    }
    /// Checked execution reachability, distinct from physical retention.
    pub const fn executable(&self) -> bool {
        self.executable
    }
    /// Fresh O formal access, present exactly for executable memory occurrences.
    pub const fn formal_access(&self) -> Option<&FormalMemoryAccess> {
        self.formal
    }
}

/// An exact transported source assertion, not a bounds or trap discharge.
#[derive(Debug)]
pub struct ScalarOutputAssertionObligationV1<'view> {
    source: SemanticKirAssertSiteV1,
    binding: &'view SemanticKirOptimizedAssertBindingV1,
}

impl ScalarOutputAssertionObligationV1<'_> {
    /// Exact source root, function and assertion block.
    pub const fn source_site(
        &self,
    ) -> (
        SemanticFunctionIdV1,
        SemanticFunctionIdV1,
        SemanticBlockIdV1,
    ) {
        (
            self.source.correspondence_owner,
            self.source.semantic_function,
            self.source.semantic_block,
        )
    }
    /// Borrows the original view's binding, including pending conditional control.
    pub const fn binding(&self) -> &SemanticKirOptimizedAssertBindingV1 {
        self.binding
    }
}

/// A compiler-generated assertion trap with complete incoming failure-edge coverage.
#[derive(Debug)]
pub struct ScalarOutputTrapObligationV1 {
    original: ScalarOutputOperationV1,
    output: Option<ScalarOutputOperationV1>,
    executable: bool,
    failure_edges: usize,
}

impl ScalarOutputTrapObligationV1 {
    /// Exact original synthetic trap occurrence.
    pub const fn original(&self) -> ScalarOutputOperationV1 {
        self.original
    }
    /// Actual retained O trap occurrence, if any.
    pub const fn output(&self) -> Option<ScalarOutputOperationV1> {
        self.output
    }
    /// A reachable trap remains an obligation; this is never a safety result.
    pub const fn executable(&self) -> bool {
        self.executable
    }
    /// Number of original incoming edges each joined to an exact source assertion.
    pub const fn failure_edges(&self) -> usize {
        self.failure_edges
    }
}

/// Authority-free coverage of scalar source/output effects and assertion obligations.
///
/// This borrows both exact checked subjects. It neither admits a final compiler
/// owner nor authenticates a target, ranked projection, launch or allocation.
/// Conditional assertions and source-rule elisions retain their original outcome;
/// no memory-completeness result discharges them. The initial closed subset is
/// one rank-one entry with scalar global loads/stores and source assertion traps.
pub struct CheckedScalarOutputObligationsV1<'view, 'source, 'output> {
    source_output: &'view ProductionSourceOutputOccurrencesV1<'source, 'output>,
    formal: &'view crate::CheckedOutputFormalMemoryAnalysisPolicy3V1<'output>,
    memory: Vec<ScalarOutputMemoryObligationV1<'view>>,
    assertions: Vec<ScalarOutputAssertionObligationV1<'view>>,
    traps: Vec<ScalarOutputTrapObligationV1>,
}

impl<'view, 'source, 'output> CheckedScalarOutputObligationsV1<'view, 'source, 'output> {
    /// Exact replayed source/output view retained by this census.
    pub const fn source_output(
        &self,
    ) -> &'view ProductionSourceOutputOccurrencesV1<'source, 'output> {
        self.source_output
    }
    /// Fresh complete memory report on the very same O owner.
    pub const fn formal(
        &self,
    ) -> &'view crate::CheckedOutputFormalMemoryAnalysisPolicy3V1<'output> {
        self.formal
    }
    /// Complete source scalar memory roster, with actual O and formal placement.
    pub fn memory(&self) -> &[ScalarOutputMemoryObligationV1<'view>] {
        &self.memory
    }
    /// All source assertion aliases and their unchanged checked outcomes.
    pub fn assertions(&self) -> &[ScalarOutputAssertionObligationV1<'view>] {
        &self.assertions
    }
    /// All source synthetic traps, including physically retained unreachable traps.
    pub fn traps(&self) -> &[ScalarOutputTrapObligationV1] {
        &self.traps
    }
    /// The census is a prerequisite only, never proof, target or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Logical storage transferred with the borrowed census and its numeric rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarOutputObligationsStorageV1(usize);
impl ScalarOutputObligationsStorageV1 {
    /// Reserve before further controlled allocation; release after dropping the census.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Derives complete scalar effect coverage from actual Policy3 source/output custody.
///
/// The view, N/B/O owners and optimizer history must already be caller-reserved.
/// The existing formal engine/report remains outside this canonical ledger.
/// New work, indexes and row capacity are charged before use. Success, error and
/// unwind restore the incoming storage floor without resetting work or failure
/// history. Unwinds become `ProductionSourceOutputErrorV1::Panicked`.
///
/// Generic coordinate preservation does not authenticate AMD target metadata;
/// the production binder and final ranked/admission checks remain separate.
pub fn derive_scalar_output_obligations_policy3_v1<'view, 'source, 'output>(
    source_output: &'view ProductionSourceOutputOccurrencesV1<'source, 'output>,
    formal: &'view crate::CheckedOutputFormalMemoryAnalysisPolicy3V1<'output>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ScalarOutputResultV1<(
    CheckedScalarOutputObligationsV1<'view, 'source, 'output>,
    ScalarOutputObligationsStorageV1,
)> {
    scalar_output_scope_v1(budget, |budget| {
        scalar_output_build_v1(source_output, formal, budget)
    })
}

fn scalar_output_scope_v1<T>(
    budget: &mut AssertOriginBudgetV1<'_>,
    build: impl FnOnce(&mut AssertOriginBudgetV1<'_>) -> ScalarOutputResultV1<T>,
) -> ScalarOutputResultV1<T> {
    let floor = budget.storage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| build(budget)))
        .unwrap_or(Err(ProductionSourceOutputErrorV1::Panicked));
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or_else(scalar_output_accounting_v1)?;
    budget
        .release_storage(release)
        .map_err(ProductionSourceOutputErrorV1::Resource)?;
    result
}

fn scalar_output_accounting_v1() -> ProductionSourceOutputErrorV1 {
    ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Accounting)
}
fn scalar_output_arithmetic_v1() -> ProductionSourceOutputErrorV1 {
    ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Arithmetic)
}
fn scalar_output_work_v1(
    budget: &mut AssertOriginBudgetV1<'_>,
    work: usize,
) -> ScalarOutputResultV1<()> {
    budget
        .charge_work(work)
        .map_err(ProductionSourceOutputErrorV1::Resource)
}
fn scalar_output_reserve_v1(
    budget: &mut AssertOriginBudgetV1<'_>,
    bytes: usize,
) -> ScalarOutputResultV1<()> {
    budget
        .reserve_storage(bytes)
        .map_err(ProductionSourceOutputErrorV1::Resource)
}
fn scalar_output_bytes_v1<T>(count: usize) -> ScalarOutputResultV1<usize> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(scalar_output_arithmetic_v1)
}
fn scalar_output_vec_v1<T>(
    count: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ScalarOutputResultV1<Vec<T>> {
    scalar_output_work_v1(budget, 3)?;
    let bytes = scalar_output_bytes_v1::<T>(count)?;
    scalar_output_reserve_v1(budget, bytes)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Allocation))?;
    let capacity = scalar_output_bytes_v1::<T>(rows.capacity())?;
    scalar_output_reserve_v1(
        budget,
        capacity
            .checked_sub(bytes)
            .ok_or_else(scalar_output_accounting_v1)?,
    )?;
    Ok(rows)
}
fn scalar_output_push_v1<T>(
    rows: &mut Vec<T>,
    row: T,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ScalarOutputResultV1<()> {
    scalar_output_work_v1(budget, 1)?;
    if rows.len() == rows.capacity() {
        return Err(ProductionSourceOutputErrorV1::Invalid(
            "scalar output census capacity exceeded",
        ));
    }
    rows.push(row);
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ScalarOutputOpcodeV1 {
    Passive,
    Memory(FormalMemoryAccessKind),
    Trap,
}

fn scalar_output_opcode_v1(
    operation: &Operation,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ScalarOutputResultV1<ScalarOutputOpcodeV1> {
    use OperationKind as Op;
    use ScalarOutputOpcodeV1 as Kind;
    scalar_output_work_v1(budget, 1)?;
    let unsupported = || {
        ProductionSourceOutputErrorV1::Invalid("scalar output opcode is outside the closed subset")
    };
    Ok(match &operation.kind {
        Op::Load { access, .. } if access.address_space == AddressSpace::Global => {
            Kind::Memory(FormalMemoryAccessKind::Read)
        }
        Op::Store { access, .. } if access.address_space == AddressSpace::Global => {
            Kind::Memory(FormalMemoryAccessKind::Write)
        }
        Op::Constant(_)
        | Op::Intrinsic(_)
        | Op::Compare { .. }
        | Op::Cast { .. }
        | Op::Select { .. }
        | Op::SliceLength { .. }
        | Op::SliceData { .. }
        | Op::GetElementPointer { .. } => Kind::Passive,
        Op::Binary { op, .. } => match op {
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Checked(_) => Kind::Passive,
            BinaryOp::Divide | BinaryOp::Remainder | BinaryOp::ShiftLeft | BinaryOp::ShiftRight => {
                return Err(unsupported());
            }
        },
        Op::Call { callee, arguments } => {
            let work = callee
                .as_str()
                .len()
                .checked_add(2)
                .and_then(|n| n.checked_mul(8))
                .ok_or_else(scalar_output_arithmetic_v1)?;
            scalar_output_work_v1(budget, work)?;
            if !arguments.is_empty()
                || !matches!(
                    AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments),
                    Some(AmdGpuDiagnosticOperation::Trap)
                )
            {
                return Err(unsupported());
            }
            Kind::Trap
        }
        Op::Execution(_)
        | Op::VerificationContract(_)
        | Op::VectorLoad(_)
        | Op::VectorStore(_)
        | Op::VectorLayoutConvert(_)
        | Op::Alloca { .. }
        | Op::Load { .. }
        | Op::Store { .. }
        | Op::GuardedLoad { .. }
        | Op::GuardedStore { .. }
        | Op::Barrier(_)
        | Op::Atomic(_)
        | Op::Fence(_)
        | Op::WorkgroupBarrier(_)
        | Op::WorkgroupMemory(_)
        | Op::Matrix(_)
        | Op::Gfx950LdsTranspose(_)
        | Op::InlineAssembly(_)
        | Op::MemoryIntrinsic(_)
        | Op::Wave(_)
        | Op::Unary { .. } => return Err(unsupported()),
    })
}

fn scalar_output_operation_index_v1(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    coordinate: ScalarOutputOperationV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ScalarOutputResultV1<usize> {
    scalar_output_work_v1(budget, 5)?;
    let invalid =
        || ProductionSourceOutputErrorV1::Invalid("scalar output operation coordinate changed");
    let function = inventory
        .functions()
        .get(coordinate.block.function.0 as usize)
        .ok_or_else(invalid)?;
    let block = inventory
        .blocks()
        .get(function.blocks.clone())
        .and_then(|rows| rows.get(coordinate.block.block as usize))
        .ok_or_else(invalid)?;
    let index = block
        .operations
        .start
        .checked_add(coordinate.operation as usize)
        .ok_or_else(scalar_output_arithmetic_v1)?;
    if !block.operations.contains(&index) || inventory.operations()[index].coordinate != coordinate
    {
        return Err(invalid());
    }
    Ok(index)
}

fn scalar_output_check_inventory_v1(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ScalarOutputResultV1<()> {
    use fe2o3_kernel_ir::KirLocalMemoryEffectRefV1 as Effect;
    for operation in inventory.operations() {
        let kind = scalar_output_opcode_v1(operation.operation, budget)?;
        scalar_output_work_v1(budget, 2)?;
        let effects = &inventory.effects()[operation.effects.clone()];
        let exact = match (kind, effects) {
            (ScalarOutputOpcodeV1::Memory(FormalMemoryAccessKind::Read), [effect]) => {
                matches!(effect.effect, Effect::Read(AddressSpace::Global))
            }
            (ScalarOutputOpcodeV1::Memory(FormalMemoryAccessKind::Write), [effect]) => {
                matches!(effect.effect, Effect::Write(AddressSpace::Global))
            }
            (ScalarOutputOpcodeV1::Passive | ScalarOutputOpcodeV1::Trap, []) => true,
            _ => false,
        };
        if !exact {
            return Err(ProductionSourceOutputErrorV1::Invalid(
                "scalar output local effect coverage changed",
            ));
        }
        if let OperationKind::Load { pointer, .. } | OperationKind::Store { pointer, .. } =
            operation.operation.kind
        {
            let definition = inventory
                .definition_for_value(operation.coordinate.block.function, pointer, budget)
                .map_err(ProductionSourceOutputErrorV1::Inventory)?
                .ok_or(ProductionSourceOutputErrorV1::Invalid(
                    "scalar memory pointer definition is absent",
                ))?;
            scalar_output_work_v1(budget, 2)?;
            if !matches!(definition.ty, Type::Pointer(pointer) if matches!(pointer.pointee.as_ref(), Type::Scalar(_)))
            {
                return Err(ProductionSourceOutputErrorV1::Invalid(
                    "scalar memory pointee is outside the closed subset",
                ));
            }
        }
        for result in &operation.operation.results {
            scalar_output_work_v1(budget, 1)?;
            if !matches!(result.ty, Type::Scalar(_) | Type::Pointer(_)) {
                return Err(ProductionSourceOutputErrorV1::Invalid(
                    "scalar output result is not scalar or pointer",
                ));
            }
        }
    }
    for block in inventory.blocks() {
        scalar_output_work_v1(budget, 2)?;
        if matches!(block.terminator, Terminator::Unreachable)
            && !block
                .block
                .operations
                .last()
                .is_some_and(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        {
            return Err(ProductionSourceOutputErrorV1::Invalid(
                "scalar output has an uncovered nonreturning path",
            ));
        }
    }
    Ok(())
}

fn scalar_output_memory_source_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    input: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    function: &SemanticKirFunctionCorrespondenceV1,
    coordinate: ScalarOutputOperationV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ScalarOutputResultV1<(SemanticKirAssertSiteV1, Option<u32>, u32)> {
    let index = scalar_output_operation_index_v1(input, coordinate, budget)?;
    let block_id = input.owner().module().functions[coordinate.block.function.0 as usize]
        .body
        .as_ref()
        .ok_or(ProductionSourceOutputErrorV1::Invalid(
            "scalar source function has no body",
        ))?
        .blocks[coordinate.block.block as usize]
        .id;
    let correspondence = &view.source.correspondence;
    let mut selected = None;
    for span in correspondence.statement_operation_spans.iter() {
        scalar_output_work_v1(budget, 5)?;
        if span.correspondence_owner != function.correspondence_owner
            || span.semantic_function != function.semantic_function
            || span.kernel_ir_block != block_id
        {
            continue;
        }
        let end = span
            .first_operation_ordinal
            .checked_add(span.operation_count)
            .ok_or_else(scalar_output_arithmetic_v1)?;
        if (span.first_operation_ordinal..end).contains(&coordinate.operation) {
            if selected.is_some() {
                return Err(ProductionSourceOutputErrorV1::Invalid(
                    "scalar source spans overlap",
                ));
            }
            selected = Some((
                span.semantic_block,
                Some(span.statement_ordinal),
                span.first_operation_ordinal,
            ));
        }
    }
    for span in correspondence.terminator_operation_spans.iter() {
        scalar_output_work_v1(budget, 5)?;
        if span.correspondence_owner != function.correspondence_owner
            || span.semantic_function != function.semantic_function
            || span.kernel_ir_block != block_id
        {
            continue;
        }
        let end = span
            .first_operation_ordinal
            .checked_add(span.operation_count)
            .ok_or_else(scalar_output_arithmetic_v1)?;
        if (span.first_operation_ordinal..end).contains(&coordinate.operation) {
            if selected.is_some() {
                return Err(ProductionSourceOutputErrorV1::Invalid(
                    "scalar source spans overlap",
                ));
            }
            selected = Some((span.semantic_block, None, span.first_operation_ordinal));
        }
    }
    let (block, statement, first) = selected.ok_or(ProductionSourceOutputErrorV1::Invalid(
        "scalar memory effect has no source span",
    ))?;
    let start = index
        .checked_sub((coordinate.operation - first) as usize)
        .ok_or_else(scalar_output_arithmetic_v1)?;
    let mut ordinal = 0u32;
    for operation in &input.operations()[start..index] {
        if matches!(
            scalar_output_opcode_v1(operation.operation, budget)?,
            ScalarOutputOpcodeV1::Memory(_)
        ) {
            ordinal = ordinal
                .checked_add(1)
                .ok_or_else(scalar_output_arithmetic_v1)?;
        }
    }
    Ok((
        SemanticKirAssertSiteV1::new(
            function.correspondence_owner,
            function.semantic_function,
            block,
        ),
        statement,
        ordinal,
    ))
}

fn scalar_output_trap_sources_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    input: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    function: &SemanticKirFunctionCorrespondenceV1,
    coordinate: ScalarOutputOperationV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ScalarOutputResultV1<usize> {
    let body = input.owner().module().functions[coordinate.block.function.0 as usize]
        .body
        .as_ref()
        .ok_or(ProductionSourceOutputErrorV1::Invalid(
            "scalar trap function has no body",
        ))?;
    let block = &body.blocks[coordinate.block.block as usize];
    scalar_output_work_v1(budget, 3)?;
    if coordinate.operation != 0
        || block.operations.len() != 1
        || !matches!(block.terminator, Some(Terminator::Unreachable))
    {
        return Err(ProductionSourceOutputErrorV1::Invalid(
            "scalar assertion trap shape changed",
        ));
    }
    let mut spans = 0usize;
    for span in view.source.correspondence.synthetic_operation_spans.iter() {
        scalar_output_work_v1(budget, 7)?;
        if span.correspondence_owner == function.correspondence_owner
            && span.semantic_function == function.semantic_function
            && span.rule == SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap
            && span.kernel_ir_block == block.id
            && span.first_operation_ordinal == 0
            && span.operation_count == 1
        {
            spans = spans
                .checked_add(1)
                .ok_or_else(scalar_output_arithmetic_v1)?;
        }
    }
    if spans != 1 {
        return Err(ProductionSourceOutputErrorV1::Invalid(
            "scalar trap has no exact synthetic source",
        ));
    }
    let mut incoming = 0usize;
    for edge in input.edges() {
        scalar_output_work_v1(budget, 1)?;
        if edge.target != coordinate.block {
            continue;
        }
        let mut matched = 0usize;
        for binding in &view.assertions.bindings {
            scalar_output_work_v1(budget, 2)?;
            if matches!(binding.import_binding().outcome(), SemanticKirAssertConditionOutcomeV1::Emitted { failure_edge, .. } if failure_edge == edge.coordinate)
            {
                matched = matched
                    .checked_add(1)
                    .ok_or_else(scalar_output_arithmetic_v1)?;
            }
        }
        if matched != 1 {
            return Err(ProductionSourceOutputErrorV1::Invalid(
                "scalar trap predecessor lacks an exact assertion obligation",
            ));
        }
        incoming = incoming
            .checked_add(1)
            .ok_or_else(scalar_output_arithmetic_v1)?;
    }
    if incoming == 0 {
        return Err(ProductionSourceOutputErrorV1::Invalid(
            "scalar trap has no source assertion predecessors",
        ));
    }
    Ok(incoming)
}

fn scalar_output_build_v1<'view, 'source, 'output>(
    view: &'view ProductionSourceOutputOccurrencesV1<'source, 'output>,
    formal: &'view crate::CheckedOutputFormalMemoryAnalysisPolicy3V1<'output>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ScalarOutputResultV1<(
    CheckedScalarOutputObligationsV1<'view, 'source, 'output>,
    ScalarOutputObligationsStorageV1,
)> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::CanonicalKirOperationOriginV1 as Origin;
    scalar_output_work_v1(budget, 7)?;
    if !matches!(
        view.checked_output,
        SourceOutputCheckedEndpointV1::OptimizerPolicy3(_)
    ) || !std::ptr::eq(view.output(), formal.output())
    {
        return Err(Error::InputCustody);
    }
    let live = view
        .source
        .retained_analysis_storage_v1()
        .checked_add(view.checked_output.storage().retained_storage())
        .and_then(|n| n.checked_add(view.storage.retained_storage()))
        .ok_or_else(scalar_output_arithmetic_v1)?;
    if budget.storage() < live {
        return Err(scalar_output_accounting_v1());
    }
    let [kernel] = view.output().module().kernels.as_slice() else {
        return Err(Error::Invalid("scalar output requires one kernel"));
    };
    let [obligations] = formal.kernels() else {
        return Err(Error::Invalid("scalar output formal roster differs"));
    };
    scalar_output_work_v1(
        budget,
        kernel
            .id
            .as_str()
            .len()
            .checked_add(kernel.entry.as_str().len())
            .and_then(|n| n.checked_add(3))
            .ok_or_else(scalar_output_arithmetic_v1)?,
    )?;
    if kernel.domain.rank() != 1
        || obligations.kernel() != &kernel.id
        || obligations.entry() != &kernel.entry
    {
        return Err(Error::Invalid(
            "scalar output formal kernel identity differs",
        ));
    }
    for function in &view.output().module().functions {
        scalar_output_work_v1(
            budget,
            function
                .id
                .as_str()
                .len()
                .checked_add(kernel.entry.as_str().len())
                .and_then(|n| n.checked_add(1))
                .ok_or_else(scalar_output_arithmetic_v1)?,
        )?;
        if function.body.is_some() && function.id != kernel.entry {
            return Err(Error::Invalid(
                "scalar output helpers are outside the closed subset",
            ));
        }
    }
    let mut source_function = None;
    for function in view.source.correspondence.lowered_functions.iter() {
        scalar_output_work_v1(
            budget,
            function
                .kernel_ir_function
                .as_str()
                .len()
                .checked_add(kernel.entry.as_str().len())
                .and_then(|n| n.checked_add(2))
                .ok_or_else(scalar_output_arithmetic_v1)?,
        )?;
        if function.kernel_ir_function == kernel.entry
            && function.role == SemanticKirFunctionRoleV1::KernelEntry
        {
            if source_function.is_some() {
                return Err(Error::Invalid("scalar output source function is ambiguous"));
            }
            source_function = Some(function);
        }
    }
    let source_function =
        source_function.ok_or(Error::Invalid("scalar output source entry is absent"))?;
    let (input, input_storage) =
        fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(view.bound(), budget)
            .map_err(Error::Inventory)?;
    scalar_output_reserve_v1(budget, input_storage.retained_storage())?;
    let (output, output_storage) =
        fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(view.output(), budget)
            .map_err(Error::Inventory)?;
    scalar_output_reserve_v1(budget, output_storage.retained_storage())?;
    let candidate = view.checked_output.occurrences().candidate();
    let (transition, transition_storage) =
        fe2o3_kernel_analysis::check_canonical_kir_transition_v1(
            &input, &output, candidate, budget,
        )
        .map_err(Error::Transition)?;
    scalar_output_reserve_v1(budget, transition_storage.retained_storage())?;
    let (control, control_storage) =
        fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1::derive(&transition, budget)
            .map_err(Error::Transition)?;
    scalar_output_reserve_v1(budget, control_storage.retained_storage())?;
    scalar_output_check_inventory_v1(&input, budget)?;
    scalar_output_check_inventory_v1(&output, budget)?;

    let header = std::mem::size_of::<CheckedScalarOutputObligationsV1<'_, '_, '_>>();
    scalar_output_reserve_v1(budget, header)?;
    let mut memory = scalar_output_vec_v1(input.effects().len(), budget)?;
    let mut assertions = scalar_output_vec_v1(view.assertions.aliases.len(), budget)?;
    let mut traps = scalar_output_vec_v1(input.calls().len(), budget)?;
    let mut placements = scalar_output_vec_v1(input.operations().len(), budget)?;
    scalar_output_work_v1(budget, input.operations().len())?;
    placements.resize(input.operations().len(), None);
    let mut used_formal = scalar_output_vec_v1(obligations.accesses().len(), budget)?;
    scalar_output_work_v1(budget, obligations.accesses().len())?;
    used_formal.resize(obligations.accesses().len(), false);
    for (ordinal, row) in candidate.operations.iter().enumerate() {
        scalar_output_work_v1(budget, 2)?;
        if let Origin::Retained(original) = row.origin {
            let index = scalar_output_operation_index_v1(&input, original, budget)?;
            if placements[index].replace(ordinal).is_some() {
                return Err(Error::Invalid(
                    "scalar input operation has duplicate output placement",
                ));
            }
        }
    }
    scalar_output_work_v1(budget, 2)?;
    let original_assertions = &view.source.assert_origins;
    if view.assertions.aliases.len() != original_assertions.aliases.len()
        || view.assertions.bindings.len() != original_assertions.bindings.len()
    {
        return Err(Error::Invalid(
            "scalar assertion reverse coverage is incomplete",
        ));
    }
    for (alias, original) in view
        .assertions
        .aliases
        .iter()
        .zip(&original_assertions.aliases)
    {
        scalar_output_work_v1(budget, 2)?;
        let binding = view
            .assertions
            .bindings
            .get(alias.binding)
            .ok_or(Error::Invalid("scalar assertion alias changed"))?;
        let original_binding = original_assertions
            .bindings
            .get(original.binding)
            .ok_or(Error::Invalid("scalar original assertion alias changed"))?;
        scalar_output_work_v1(budget, 4)?;
        if alias.site != original.site || binding.import_binding() != *original_binding {
            return Err(Error::Invalid("scalar assertion source binding changed"));
        }
        if matches!(
            binding.outcome(),
            SemanticKirOptimizedAssertOutcomeV1::SelectedFailure { .. }
        ) {
            return Err(Error::Invalid(
                "scalar output selects source assertion failure",
            ));
        }
        scalar_output_push_v1(
            &mut assertions,
            ScalarOutputAssertionObligationV1 {
                source: alias.site,
                binding,
            },
            budget,
        )?;
    }
    let mut matched_output_effects = 0usize;
    let mut matched_output_calls = 0usize;
    for (index, operation) in input.operations().iter().enumerate() {
        let kind = scalar_output_opcode_v1(operation.operation, budget)?;
        if kind == ScalarOutputOpcodeV1::Passive {
            continue;
        }
        let executable = control
            .block(operation.coordinate.block, budget)
            .map_err(Error::Transition)?
            .reachable;
        scalar_output_work_v1(budget, 2)?;
        let placement = placements[index].map(|ordinal| &output.operations()[ordinal]);
        if executable && placement.is_none() {
            return Err(Error::Invalid("scalar executable effect was omitted"));
        }
        match kind {
            ScalarOutputOpcodeV1::Memory(access_kind) => {
                let (source, statement, access_ordinal) = scalar_output_memory_source_v1(
                    view,
                    &input,
                    source_function,
                    operation.coordinate,
                    budget,
                )?;
                let mut actual = None;
                if let Some(placed) = placement {
                    matched_output_effects = matched_output_effects
                        .checked_add(1)
                        .ok_or_else(scalar_output_arithmetic_v1)?;
                    if scalar_output_opcode_v1(placed.operation, budget)? != kind {
                        return Err(Error::Invalid("scalar output access kind changed"));
                    }
                    let output_block = &view.output().module().functions
                        [placed.coordinate.block.function.0 as usize]
                        .body
                        .as_ref()
                        .ok_or(Error::Invalid("scalar output entry has no body"))?
                        .blocks[placed.coordinate.block.block as usize];
                    for (ordinal, access) in obligations.accesses().iter().enumerate() {
                        scalar_output_work_v1(budget, 5)?;
                        if access.location()
                            == FunctionOperationLocation::new(
                                output_block.id,
                                placed.coordinate.operation as usize,
                            )
                        {
                            if !executable
                                || actual.is_some()
                                || used_formal[ordinal]
                                || access.kind() != access_kind
                                || access.address_space() != AddressSpace::Global
                            {
                                return Err(Error::Invalid(
                                    "scalar output formal access coverage differs",
                                ));
                            }
                            used_formal[ordinal] = true;
                            actual = Some(access);
                        }
                    }
                }
                if executable != actual.is_some() {
                    return Err(Error::Invalid(
                        "scalar executable access lacks fresh formal coverage",
                    ));
                }
                scalar_output_push_v1(
                    &mut memory,
                    ScalarOutputMemoryObligationV1 {
                        source,
                        statement,
                        access_ordinal,
                        original: operation.coordinate,
                        output: placement.map(|row| row.coordinate),
                        executable,
                        formal: actual,
                    },
                    budget,
                )?;
            }
            ScalarOutputOpcodeV1::Trap => {
                let failure_edges = scalar_output_trap_sources_v1(
                    view,
                    &input,
                    source_function,
                    operation.coordinate,
                    budget,
                )?;
                if let Some(placed) = placement {
                    matched_output_calls = matched_output_calls
                        .checked_add(1)
                        .ok_or_else(scalar_output_arithmetic_v1)?;
                    if scalar_output_opcode_v1(placed.operation, budget)? != kind {
                        return Err(Error::Invalid("scalar output trap identity changed"));
                    }
                }
                scalar_output_push_v1(
                    &mut traps,
                    ScalarOutputTrapObligationV1 {
                        original: operation.coordinate,
                        output: placement.map(|row| row.coordinate),
                        executable,
                        failure_edges,
                    },
                    budget,
                )?;
            }
            ScalarOutputOpcodeV1::Passive => unreachable!(),
        }
    }
    for used in used_formal {
        scalar_output_work_v1(budget, 1)?;
        if !used {
            return Err(Error::Invalid(
                "scalar formal access has no output occurrence",
            ));
        }
    }
    scalar_output_work_v1(budget, 5)?;
    if matched_output_effects != output.effects().len()
        || matched_output_calls != output.calls().len()
    {
        return Err(Error::Invalid(
            "scalar output reverse effect coverage is incomplete",
        ));
    }
    let retained = header
        .checked_add(
            scalar_output_bytes_v1::<ScalarOutputMemoryObligationV1<'_>>(memory.capacity())?,
        )
        .and_then(|n| {
            n.checked_add(
                assertions
                    .capacity()
                    .checked_mul(std::mem::size_of::<ScalarOutputAssertionObligationV1<'_>>())?,
            )
        })
        .and_then(|n| {
            n.checked_add(
                traps
                    .capacity()
                    .checked_mul(std::mem::size_of::<ScalarOutputTrapObligationV1>())?,
            )
        })
        .ok_or_else(scalar_output_arithmetic_v1)?;
    Ok((
        CheckedScalarOutputObligationsV1 {
            source_output: view,
            formal,
            memory,
            assertions,
            traps,
        },
        ScalarOutputObligationsStorageV1(retained),
    ))
}

#[cfg(test)]
#[path = "production_scalar_output_obligations_policy3_v1_tests.rs"]
mod scalar_output_obligations_policy3_v1_tests;
