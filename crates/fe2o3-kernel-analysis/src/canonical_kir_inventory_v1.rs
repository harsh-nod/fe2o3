//! Borrowed inventory of one exact connected canonical V12 owner.
//! Construction indexes the executable graph, never constructs another program.
//! Reports here establish no bounds, alias, initialization or semantic-preservation proof.

use std::{cmp::Ordering, error::Error, fmt, mem::size_of, ops::Range};

use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirAccessCoordinateV1 as Access, CanonicalKirBlockCoordinateV1 as Block,
    CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirEdgeArgumentCoordinateV1 as EdgeArgument, CanonicalKirEdgeCoordinateV1 as Edge,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    CanonicalKirOperationCoordinateV1 as OperationCoordinate, CanonicalKirUseCoordinateV1 as Use,
    CompilerOrderingEffectSummaryV12, Function, Kernel, KirLocalMemoryEffectRefV1, Operation,
    OperationKind, Terminator, Type, ValueId, VerifiedCanonicalKernelIrIdentityV12,
    VerifiedCanonicalKernelIrModuleV12,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirInventoryErrorV1 {
    Resource(Resource),
    /// A trusted connected-owner invariant or this inventory's bookkeeping failed.
    InconsistentOwner,
}
impl From<Resource> for CanonicalKirInventoryErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for CanonicalKirInventoryErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::InconsistentOwner => f.write_str("canonical inventory owner/roster mismatch"),
        }
    }
}
impl Error for CanonicalKirInventoryErrorV1 {}
type Result<T> = std::result::Result<T, CanonicalKirInventoryErrorV1>;

// The census admits all vector payloads. A counting defect must reject before
// an unmetered push could grow a vector.
macro_rules! append {
    ($vector:expr, $admitted:expr, $item:expr) => {{
        let admitted = $admitted;
        let item = $item;
        let vector = &mut $vector;
        if vector.len() >= admitted || vector.len() == vector.capacity() {
            return Err(CanonicalKirInventoryErrorV1::InconsistentOwner);
        }
        vector.push(item);
    }};
}

/// This inventory does not analyze traps or convergence. Kept separate from
/// both physical effects and ordered compiler-contract effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirBehaviorAnalysisV1 {
    NotAnalyzed,
}

#[derive(Debug)]
pub struct CanonicalKirFunctionRefV1<'g> {
    pub coordinate: FunctionCoordinate,
    pub function: &'g Function,
    pub blocks: Range<usize>,
    pub definitions: Range<usize>,
    pub operations: Range<usize>,
    pub uses: Range<usize>,
    pub edges: Range<usize>,
    pub edge_arguments: Range<usize>,
    pub effects: Range<usize>,
    pub calls: Range<usize>,
}
#[derive(Debug)]
pub struct CanonicalKirBlockRefV1<'g> {
    pub coordinate: Block,
    pub block: &'g BasicBlock,
    pub terminator: &'g Terminator,
    pub parameters: Range<usize>,
    pub operations: Range<usize>,
    pub terminator_uses: Range<usize>,
    pub edges: Range<usize>,
}
#[derive(Debug)]
pub struct CanonicalKirDefinitionRefV1<'g> {
    pub coordinate: Definition,
    /// None only for an external declaration's signature argument.
    pub value: Option<ValueId>,
    pub ty: &'g Type,
}
#[derive(Debug)]
pub struct CanonicalKirOperationRefV1<'g> {
    pub coordinate: OperationCoordinate,
    pub operation: &'g Operation,
    pub results: Range<usize>,
    pub operands: Range<usize>,
    pub effects: Range<usize>,
}
impl CanonicalKirOperationRefV1<'_> {
    pub fn compiler_ordering(&self) -> CompilerOrderingEffectSummaryV12 {
        self.operation.compiler_ordering_effects_v12()
    }
    pub const fn traps(&self) -> CanonicalKirBehaviorAnalysisV1 {
        CanonicalKirBehaviorAnalysisV1::NotAnalyzed
    }
    pub const fn convergence(&self) -> CanonicalKirBehaviorAnalysisV1 {
        CanonicalKirBehaviorAnalysisV1::NotAnalyzed
    }
}
#[derive(Debug)]
pub struct CanonicalKirUseRefV1 {
    pub coordinate: Use,
    pub value: ValueId,
    /// Dense index in definitions(), resolved from the exact function-local ID.
    pub definition: usize,
}
#[derive(Debug)]
pub struct CanonicalKirEdgeRefV1<'g> {
    pub coordinate: Edge,
    pub target: Block,
    pub target_id: BlockId,
    /// Original executable edge payload, never copied/reordered.
    pub arguments: &'g [ValueId],
    pub bindings: Range<usize>,
}
#[derive(Debug)]
pub struct CanonicalKirEdgeArgumentRefV1 {
    pub coordinate: EdgeArgument,
    pub value: ValueId,
    pub incoming_definition: usize,
    pub target_definition: usize,
}
#[derive(Debug)]
pub struct CanonicalKirEffectRefV1<'g> {
    pub coordinate: Access,
    pub operation: &'g Operation,
    pub effect: KirLocalMemoryEffectRefV1<'g>,
}
#[derive(Debug)]
pub struct CanonicalKirCallRefV1<'g> {
    pub coordinate: OperationCoordinate,
    pub operation: &'g Operation,
    pub callee: &'g str,
    /// None for a verified reserved builtin not present in the function roster.
    /// Some may identify a declaration. Neither case is a transitive summary.
    pub target: Option<FunctionCoordinate>,
}
#[derive(Debug)]
pub struct CanonicalKirKernelRefV1<'g> {
    pub ordinal: u32,
    pub kernel: &'g Kernel,
    pub entry: FunctionCoordinate,
}

/// Logical retained payload, not allocator capacity, overhead, RSS or authority.
/// Reserve this amount before another ledger-controlled allocation while the
/// inventory lives; release only after dropping the inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirInventoryStorageV1 {
    retained: usize,
}
impl CanonicalKirInventoryStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

/// Complete immutable inventory. A borrow retains the exact connected owner;
/// there is no constructor taking an arbitrary module plus a claimed hash.
/// Indexes are facts about that graph, not a second executable representation.
#[derive(Debug)]
pub struct CanonicalKirInventoryV1<'g> {
    owner: &'g VerifiedCanonicalKernelIrModuleV12,
    functions: Vec<CanonicalKirFunctionRefV1<'g>>,
    blocks: Vec<CanonicalKirBlockRefV1<'g>>,
    definitions: Vec<CanonicalKirDefinitionRefV1<'g>>,
    operations: Vec<CanonicalKirOperationRefV1<'g>>,
    uses: Vec<CanonicalKirUseRefV1>,
    edges: Vec<CanonicalKirEdgeRefV1<'g>>,
    edge_arguments: Vec<CanonicalKirEdgeArgumentRefV1>,
    effects: Vec<CanonicalKirEffectRefV1<'g>>,
    calls: Vec<CanonicalKirCallRefV1<'g>>,
    kernels: Vec<CanonicalKirKernelRefV1<'g>>,
    function_index: Vec<(&'g str, FunctionCoordinate)>,
    block_index: Vec<(Block, BlockId, usize)>,
    value_index: Vec<(FunctionCoordinate, ValueId, usize)>,
}
impl<'g> CanonicalKirInventoryV1<'g> {
    /// Charges before every roster/operand/effect/edge visit, allocation,
    /// index comparison/swap, and linking operation. Two source traversals,
    /// fallible in-place heapsort and binary search have deterministic failure
    /// prefixes. No dynamic type payload is copied or traversed.
    ///
    /// Storage counts size_of(Self) plus requested vector record payloads.
    /// Existing source/connected bytes, allocator slack and caller diagnostics
    /// are excluded. Success transfers the returned payload and restores the
    /// incoming floor; all errors drop candidates before restoring that floor.
    /// Accepted work, peak and first-failure history remain in the ledger.
    pub fn derive(
        owner: &'g VerifiedCanonicalKernelIrModuleV12,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CanonicalKirInventoryStorageV1)> {
        let floor = budget.storage();
        let result = Self::build(owner, budget);
        let retained = budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?;
        budget.release_storage(retained)?;
        result.map(|inventory| (inventory, CanonicalKirInventoryStorageV1 { retained }))
    }

    /// Recompute this exact inventory's logical retained receipt from immutable
    /// row/index lengths. Names/types/operations are borrowed, not copied; their
    /// bytes remain in the connected owner's separate reservation.
    /// This adds no reservation and changes no derive/allocation semantics.
    /// The fixed thirteen vector headers are charged before inspection.
    pub fn retained_storage_v1(&self, budget: &mut Budget<'_>) -> Result<usize> {
        budget.charge_work(14)?;
        let mut bytes = size_of::<Self>();
        macro_rules! retained {
            ($rows:expr) => {
                bytes = bytes
                    .checked_add(retained_row_storage($rows)?)
                    .ok_or(Resource::Arithmetic)?;
            };
        }
        retained!(&self.functions);
        retained!(&self.blocks);
        retained!(&self.definitions);
        retained!(&self.operations);
        retained!(&self.uses);
        retained!(&self.edges);
        retained!(&self.edge_arguments);
        retained!(&self.effects);
        retained!(&self.calls);
        retained!(&self.kernels);
        retained!(&self.function_index);
        retained!(&self.block_index);
        retained!(&self.value_index);
        Ok(bytes)
    }

    pub const fn owner(&self) -> &'g VerifiedCanonicalKernelIrModuleV12 {
        self.owner
    }
    pub fn identity(&self) -> VerifiedCanonicalKernelIrIdentityV12 {
        *self.owner.canonical().identity()
    }
    /// Ephemeral exact-owner comparison, not a durable pointer identity.
    pub fn belongs_to(&self, owner: &VerifiedCanonicalKernelIrModuleV12) -> bool {
        std::ptr::eq(self.owner, owner)
    }
    pub fn functions(&self) -> &[CanonicalKirFunctionRefV1<'g>] {
        &self.functions
    }
    pub fn blocks(&self) -> &[CanonicalKirBlockRefV1<'g>] {
        &self.blocks
    }
    pub fn definitions(&self) -> &[CanonicalKirDefinitionRefV1<'g>] {
        &self.definitions
    }
    /// Metered exact-name query through the already derived function index.
    pub fn function_for_name(
        &self,
        name: &str,
        budget: &mut Budget<'_>,
    ) -> Result<Option<&CanonicalKirFunctionRefV1<'g>>> {
        Ok(find_function(&self.function_index, name, budget)?
            .and_then(|coordinate| self.functions.get(coordinate.0 as usize)))
    }

    /// Metered function-qualified sparse BlockId query without a graph rescan.
    pub fn block_for_id(
        &self,
        function: FunctionCoordinate,
        block: BlockId,
        budget: &mut Budget<'_>,
    ) -> Result<Option<&CanonicalKirBlockRefV1<'g>>> {
        Ok(find(&self.block_index, budget, |row, budget| {
            budget.charge_work(1)?;
            Ok((row.0.function, row.1).cmp(&(function, block)))
        })?
        .and_then(|position| self.blocks.get(self.block_index[position].2)))
    }
    pub fn operations(&self) -> &[CanonicalKirOperationRefV1<'g>] {
        &self.operations
    }
    pub fn uses(&self) -> &[CanonicalKirUseRefV1] {
        &self.uses
    }
    pub fn edges(&self) -> &[CanonicalKirEdgeRefV1<'g>] {
        &self.edges
    }
    pub fn edge_arguments(&self) -> &[CanonicalKirEdgeArgumentRefV1] {
        &self.edge_arguments
    }
    pub fn effects(&self) -> &[CanonicalKirEffectRefV1<'g>] {
        &self.effects
    }
    pub fn calls(&self) -> &[CanonicalKirCallRefV1<'g>] {
        &self.calls
    }
    pub fn kernels(&self) -> &[CanonicalKirKernelRefV1<'g>] {
        &self.kernels
    }

    /// Metered lookup; raw sparse IDs never determine allocation sizes.
    pub fn definition_for_value(
        &self,
        function: FunctionCoordinate,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> Result<Option<&CanonicalKirDefinitionRefV1<'g>>> {
        Ok(self
            .definition_index_for_value(function, value, budget)?
            .and_then(|index| self.definitions.get(index)))
    }

    /// Index into this inventory's definition roster, not a durable identity.
    /// Prepared analyses use this to index dense facts without a second search.
    pub fn definition_index_for_value(
        &self,
        function: FunctionCoordinate,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> Result<Option<usize>> {
        find_value(&self.value_index, function, value, budget)
    }

    fn build(
        owner: &'g VerifiedCanonicalKernelIrModuleV12,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        let counts = census(owner, budget)?;
        budget.reserve_storage(size_of::<Self>())?;
        let mut result = Self {
            owner,
            functions: allocate(counts.functions, budget)?,
            blocks: allocate(counts.blocks, budget)?,
            definitions: allocate(counts.definitions, budget)?,
            operations: allocate(counts.operations, budget)?,
            uses: allocate(counts.uses, budget)?,
            edges: allocate(counts.edges, budget)?,
            edge_arguments: allocate(counts.edge_arguments, budget)?,
            effects: allocate(counts.effects, budget)?,
            calls: allocate(counts.calls, budget)?,
            kernels: allocate(counts.kernels, budget)?,
            function_index: allocate(counts.functions, budget)?,
            block_index: allocate(counts.blocks, budget)?,
            value_index: allocate(counts.values, budget)?,
        };
        result.fill(&counts, budget)?;
        result.link(&counts, budget)?;
        Ok(result)
    }

    fn fill(&mut self, counts: &Census, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(1)?;
        let owner = self.owner;
        for (function_ordinal, function) in owner.module().functions.iter().enumerate() {
            budget.charge_work(1)?;
            let f = FunctionCoordinate(ordinal(function_ordinal)?);
            let starts = self.positions();
            for (argument, ty) in function.signature.parameters.iter().enumerate() {
                budget.charge_work(1)?;
                let value = function
                    .body
                    .as_ref()
                    .map(|body| {
                        body.parameters
                            .get(argument)
                            .copied()
                            .ok_or(CanonicalKirInventoryErrorV1::InconsistentOwner)
                    })
                    .transpose()?;
                append!(
                    self.definitions,
                    counts.definitions,
                    CanonicalKirDefinitionRefV1 {
                        coordinate: Definition::FunctionArgument {
                            function: f,
                            argument: ordinal(argument)?
                        },
                        value,
                        ty,
                    }
                );
            }
            if let Some(body) = &function.body {
                for (block_ordinal, block) in body.blocks.iter().enumerate() {
                    budget.charge_work(1)?;
                    let b = Block {
                        function: f,
                        block: ordinal(block_ordinal)?,
                    };
                    let parameter_start = self.definitions.len();
                    for (argument, parameter) in block.parameters.iter().enumerate() {
                        budget.charge_work(1)?;
                        append!(
                            self.definitions,
                            counts.definitions,
                            CanonicalKirDefinitionRefV1 {
                                coordinate: Definition::BlockArgument {
                                    block: b,
                                    argument: ordinal(argument)?
                                },
                                value: Some(parameter.id),
                                ty: &parameter.ty,
                            }
                        );
                    }
                    let parameters = parameter_start..self.definitions.len();
                    let operation_start = self.operations.len();
                    for (operation_ordinal, operation) in block.operations.iter().enumerate() {
                        budget.charge_work(1)?;
                        let coordinate = OperationCoordinate {
                            block: b,
                            operation: ordinal(operation_ordinal)?,
                        };
                        let result_start = self.definitions.len();
                        for (result, definition) in operation.results.iter().enumerate() {
                            budget.charge_work(1)?;
                            append!(
                                self.definitions,
                                counts.definitions,
                                CanonicalKirDefinitionRefV1 {
                                    coordinate: Definition::Result {
                                        operation: coordinate,
                                        result: ordinal(result)?
                                    },
                                    value: Some(definition.id),
                                    ty: &definition.ty,
                                }
                            );
                        }
                        let operand_start = self.uses.len();
                        operation.kind.try_visit_operands(|value| -> Result<()> {
                            budget.charge_work(1)?;
                            let operand = ordinal(self.uses.len() - operand_start)?;
                            append!(
                                self.uses,
                                counts.uses,
                                CanonicalKirUseRefV1 {
                                    coordinate: Use::OperationOperand {
                                        operation: coordinate,
                                        operand
                                    },
                                    value,
                                    definition: usize::MAX,
                                }
                            );
                            Ok(())
                        })?;
                        let effect_start = self.effects.len();
                        operation.try_visit_local_memory_effects_v1(|effect| -> Result<()> {
                            budget.charge_work(1)?;
                            let effect_ordinal = ordinal(self.effects.len() - effect_start)?;
                            append!(
                                self.effects,
                                counts.effects,
                                CanonicalKirEffectRefV1 {
                                    coordinate: Access {
                                        operation: coordinate,
                                        effect: effect_ordinal
                                    },
                                    operation,
                                    effect,
                                }
                            );
                            Ok(())
                        })?;
                        if let OperationKind::Call { callee, .. } = &operation.kind {
                            budget.charge_work(1)?;
                            append!(
                                self.calls,
                                counts.calls,
                                CanonicalKirCallRefV1 {
                                    coordinate,
                                    operation,
                                    callee: callee.as_str(),
                                    target: None,
                                }
                            );
                        }
                        append!(
                            self.operations,
                            counts.operations,
                            CanonicalKirOperationRefV1 {
                                coordinate,
                                operation,
                                results: result_start..self.definitions.len(),
                                operands: operand_start..self.uses.len(),
                                effects: effect_start..self.effects.len(),
                            }
                        );
                    }
                    budget.charge_work(1)?;
                    let terminator = block
                        .terminator
                        .as_ref()
                        .ok_or(CanonicalKirInventoryErrorV1::InconsistentOwner)?;
                    let use_start = self.uses.len();
                    terminator.try_visit_operands(|value| -> Result<()> {
                        budget.charge_work(1)?;
                        let operand = ordinal(self.uses.len() - use_start)?;
                        append!(
                            self.uses,
                            counts.uses,
                            CanonicalKirUseRefV1 {
                                coordinate: Use::TerminatorOperand { block: b, operand },
                                value,
                                definition: usize::MAX,
                            }
                        );
                        Ok(())
                    })?;
                    let edge_start = self.edges.len();
                    terminator.try_visit_edges_v1(|target_id, arguments| -> Result<()> {
                        budget.charge_work(1)?;
                        let coordinate = Edge {
                            source: b,
                            successor: ordinal(self.edges.len() - edge_start)?,
                        };
                        let argument_start = self.edge_arguments.len();
                        for (argument, value) in arguments.iter().copied().enumerate() {
                            budget.charge_work(1)?;
                            append!(
                                self.edge_arguments,
                                counts.edge_arguments,
                                CanonicalKirEdgeArgumentRefV1 {
                                    coordinate: EdgeArgument {
                                        edge: coordinate,
                                        argument: ordinal(argument)?
                                    },
                                    value,
                                    incoming_definition: usize::MAX,
                                    target_definition: usize::MAX,
                                }
                            );
                        }
                        append!(
                            self.edges,
                            counts.edges,
                            CanonicalKirEdgeRefV1 {
                                coordinate,
                                target: b,
                                target_id,
                                arguments,
                                bindings: argument_start..self.edge_arguments.len(),
                            }
                        );
                        Ok(())
                    })?;
                    append!(
                        self.blocks,
                        counts.blocks,
                        CanonicalKirBlockRefV1 {
                            coordinate: b,
                            block,
                            terminator,
                            parameters,
                            operations: operation_start..self.operations.len(),
                            terminator_uses: use_start..self.uses.len(),
                            edges: edge_start..self.edges.len(),
                        }
                    );
                }
            }
            let ends = self.positions();
            append!(
                self.functions,
                counts.functions,
                CanonicalKirFunctionRefV1 {
                    coordinate: f,
                    function,
                    blocks: starts[0]..ends[0],
                    definitions: starts[1]..ends[1],
                    operations: starts[2]..ends[2],
                    uses: starts[3]..ends[3],
                    edges: starts[4]..ends[4],
                    edge_arguments: starts[5]..ends[5],
                    effects: starts[6]..ends[6],
                    calls: starts[7]..ends[7],
                }
            );
        }
        for (index, kernel) in owner.module().kernels.iter().enumerate() {
            budget.charge_work(1)?;
            append!(
                self.kernels,
                counts.kernels,
                CanonicalKirKernelRefV1 {
                    ordinal: ordinal(index)?,
                    kernel,
                    entry: FunctionCoordinate(0),
                }
            );
        }
        Ok(())
    }

    fn positions(&self) -> [usize; 8] {
        [
            self.blocks.len(),
            self.definitions.len(),
            self.operations.len(),
            self.uses.len(),
            self.edges.len(),
            self.edge_arguments.len(),
            self.effects.len(),
            self.calls.len(),
        ]
    }

    fn link(&mut self, counts: &Census, budget: &mut Budget<'_>) -> Result<()> {
        for function in &self.functions {
            budget.charge_work(1)?;
            append!(
                self.function_index,
                counts.functions,
                (function.function.id.as_str(), function.coordinate)
            );
        }
        for (index, block) in self.blocks.iter().enumerate() {
            budget.charge_work(1)?;
            append!(
                self.block_index,
                counts.blocks,
                (block.coordinate, block.block.id, index)
            );
        }
        for (index, definition) in self.definitions.iter().enumerate() {
            budget.charge_work(1)?;
            if let Some(value) = definition.value {
                let function = definition_function(definition.coordinate);
                append!(self.value_index, counts.values, (function, value, index));
            }
        }
        heap_sort(&mut self.function_index, budget, |a, b, budget| {
            text_compare(a.0, b.0, budget)
        })?;
        heap_sort(&mut self.block_index, budget, |a, b, budget| {
            budget.charge_work(1)?;
            Ok((a.0.function, a.1).cmp(&(b.0.function, b.1)))
        })?;
        heap_sort(&mut self.value_index, budget, |a, b, budget| {
            budget.charge_work(1)?;
            Ok((a.0, a.1).cmp(&(b.0, b.1)))
        })?;
        for operand in &mut self.uses {
            budget.charge_work(1)?;
            let function = match operand.coordinate {
                Use::OperationOperand { operation, .. } => operation.block.function,
                Use::TerminatorOperand { block, .. } => block.function,
            };
            operand.definition = find_value(&self.value_index, function, operand.value, budget)?
                .ok_or(CanonicalKirInventoryErrorV1::InconsistentOwner)?;
        }
        for edge in &mut self.edges {
            budget.charge_work(1)?;
            let index = find(&self.block_index, budget, |row, budget| {
                budget.charge_work(1)?;
                Ok((row.0.function, row.1).cmp(&(edge.coordinate.source.function, edge.target_id)))
            })?
            .ok_or(CanonicalKirInventoryErrorV1::InconsistentOwner)?;
            let block = &self.blocks[self.block_index[index].2];
            edge.target = block.coordinate;
            if edge.arguments.len() != block.parameters.len() {
                return Err(CanonicalKirInventoryErrorV1::InconsistentOwner);
            }
            for (offset, binding) in self.edge_arguments[edge.bindings.clone()]
                .iter_mut()
                .enumerate()
            {
                budget.charge_work(1)?;
                binding.incoming_definition = find_value(
                    &self.value_index,
                    edge.coordinate.source.function,
                    binding.value,
                    budget,
                )?
                .ok_or(CanonicalKirInventoryErrorV1::InconsistentOwner)?;
                binding.target_definition = block
                    .parameters
                    .start
                    .checked_add(offset)
                    .ok_or(Resource::Arithmetic)?;
            }
        }
        for call in &mut self.calls {
            budget.charge_work(1)?;
            call.target = find_function(&self.function_index, call.callee, budget)?;
        }
        for kernel in &mut self.kernels {
            budget.charge_work(1)?;
            kernel.entry =
                find_function(&self.function_index, kernel.kernel.entry.as_str(), budget)?
                    .ok_or(CanonicalKirInventoryErrorV1::InconsistentOwner)?;
        }
        Ok(())
    }
}

fn definition_function(value: Definition) -> FunctionCoordinate {
    match value {
        Definition::FunctionArgument { function, .. } => function,
        Definition::BlockArgument { block, .. } => block.function,
        Definition::Result { operation, .. } => operation.block.function,
    }
}

#[derive(Default)]
struct Census {
    functions: usize,
    blocks: usize,
    definitions: usize,
    values: usize,
    operations: usize,
    uses: usize,
    edges: usize,
    edge_arguments: usize,
    effects: usize,
    calls: usize,
    kernels: usize,
}
fn count(value: &mut usize, budget: &mut Budget<'_>) -> Result<()> {
    budget.charge_work(1)?;
    *value = value.checked_add(1).ok_or(Resource::Arithmetic)?;
    Ok(())
}
fn census(owner: &VerifiedCanonicalKernelIrModuleV12, budget: &mut Budget<'_>) -> Result<Census> {
    budget.charge_work(1)?;
    let mut c = Census::default();
    for function in &owner.module().functions {
        count(&mut c.functions, budget)?;
        for _ in &function.signature.parameters {
            count(&mut c.definitions, budget)?;
            if function.body.is_some() {
                c.values = c.values.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
        }
        if let Some(body) = &function.body {
            for block in &body.blocks {
                count(&mut c.blocks, budget)?;
                for _ in &block.parameters {
                    count(&mut c.definitions, budget)?;
                    c.values = c.values.checked_add(1).ok_or(Resource::Arithmetic)?;
                }
                for operation in &block.operations {
                    count(&mut c.operations, budget)?;
                    for _ in &operation.results {
                        count(&mut c.definitions, budget)?;
                        c.values = c.values.checked_add(1).ok_or(Resource::Arithmetic)?;
                    }
                    operation
                        .kind
                        .try_visit_operands(|_| count(&mut c.uses, budget))?;
                    operation
                        .try_visit_local_memory_effects_v1(|_| count(&mut c.effects, budget))?;
                    if matches!(operation.kind, OperationKind::Call { .. }) {
                        count(&mut c.calls, budget)?;
                    }
                }
                budget.charge_work(1)?;
                let terminator = block
                    .terminator
                    .as_ref()
                    .ok_or(CanonicalKirInventoryErrorV1::InconsistentOwner)?;
                terminator.try_visit_operands(|_| count(&mut c.uses, budget))?;
                terminator.try_visit_edges_v1(|_, arguments| -> Result<()> {
                    count(&mut c.edges, budget)?;
                    for _ in arguments {
                        count(&mut c.edge_arguments, budget)?;
                    }
                    Ok(())
                })?;
            }
        }
    }
    for _ in &owner.module().kernels {
        count(&mut c.kernels, budget)?;
    }
    Ok(c)
}
fn ordinal(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| Resource::Arithmetic.into())
}
fn row_storage<T>(count: usize) -> Result<usize> {
    count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic.into())
}
fn retained_row_storage<T>(rows: &[T]) -> Result<usize> {
    row_storage::<T>(rows.len())
}
fn allocate<T>(count: usize, budget: &mut Budget<'_>) -> Result<Vec<T>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    budget.charge_work(1)?;
    budget.reserve_storage(row_storage::<T>(count)?)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    Ok(values)
}
fn text_compare(a: &str, b: &str, budget: &mut Budget<'_>) -> Result<Ordering> {
    budget.charge_work(
        a.len()
            .min(b.len())
            .checked_add(1)
            .ok_or(Resource::Arithmetic)?,
    )?;
    Ok(a.cmp(b))
}
fn find<T>(
    values: &[T],
    budget: &mut Budget<'_>,
    mut compare: impl FnMut(&T, &mut Budget<'_>) -> Result<Ordering>,
) -> Result<Option<usize>> {
    let (mut low, mut high) = (0, values.len());
    while low < high {
        budget.charge_work(1)?;
        let middle = low + (high - low) / 2;
        match compare(&values[middle], budget)? {
            Ordering::Less => low = middle + 1,
            Ordering::Greater => high = middle,
            Ordering::Equal => return Ok(Some(middle)),
        }
    }
    Ok(None)
}
fn find_value(
    index: &[(FunctionCoordinate, ValueId, usize)],
    function: FunctionCoordinate,
    value: ValueId,
    budget: &mut Budget<'_>,
) -> Result<Option<usize>> {
    Ok(find(index, budget, |row, budget| {
        budget.charge_work(1)?;
        Ok((row.0, row.1).cmp(&(function, value)))
    })?
    .map(|position| index[position].2))
}
fn find_function(
    index: &[(&str, FunctionCoordinate)],
    name: &str,
    budget: &mut Budget<'_>,
) -> Result<Option<FunctionCoordinate>> {
    Ok(find(index, budget, |row, budget| {
        text_compare(row.0, name, budget)
    })?
    .map(|position| index[position].1))
}

/// Fallible in-place heapsort permits charging each comparison/swap before it
/// occurs, rather than assuming an implementation-specific std sorting bound.
fn heap_sort<T>(
    values: &mut [T],
    budget: &mut Budget<'_>,
    mut compare: impl FnMut(&T, &T, &mut Budget<'_>) -> Result<Ordering>,
) -> Result<()> {
    fn sift<T>(
        values: &mut [T],
        mut root: usize,
        end: usize,
        budget: &mut Budget<'_>,
        compare: &mut impl FnMut(&T, &T, &mut Budget<'_>) -> Result<Ordering>,
    ) -> Result<()> {
        loop {
            budget.charge_work(1)?;
            let child = root
                .checked_mul(2)
                .and_then(|n| n.checked_add(1))
                .ok_or(Resource::Arithmetic)?;
            if child >= end {
                return Ok(());
            }
            let next = if child + 1 < end
                && compare(&values[child], &values[child + 1], budget)? == Ordering::Less
            {
                child + 1
            } else {
                child
            };
            if compare(&values[root], &values[next], budget)? != Ordering::Less {
                return Ok(());
            }
            budget.charge_work(1)?;
            values.swap(root, next);
            root = next;
        }
    }
    if values.len() < 2 {
        return Ok(());
    }
    let length = values.len();
    for root in (0..length / 2).rev() {
        sift(values, root, length, budget, &mut compare)?;
    }
    for end in (1..values.len()).rev() {
        budget.charge_work(1)?;
        values.swap(0, end);
        sift(values, 0, end, budget, &mut compare)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "canonical_kir_inventory_v1_tests.rs"]
mod tests;
