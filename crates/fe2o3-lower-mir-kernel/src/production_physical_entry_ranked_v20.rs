//! Fixed safety projection of the actual KIR20 body, never executable code.
//! Exact source/canonical replay and typed physical provenance remain separate
//! mandatory owners. Pointer words are opaque dependencies, not fabricated
//! numerical addresses. Only the authored masked store becomes analysis CFG.
use super::*;
use dialect_kernel::{AccessKindAttr as Access, IndexBinaryKindAttr as Binary};
use fe2o3_kernel_ir::{
    Gfx942PhysicalEntryOpcodeV20 as Opcode, OperationKind as Op,
    PhysicalEntryKernargSlotV20 as Slot, PhysicalEntryMemoryObligationsV20, Terminator as End,
    VerifiedCanonicalKernelIrModuleV20,
};
use fe2o3_pliron::{
    ProductionRankedBlockV1 as Block, ProductionRankedKernelV1 as Recipe,
    ProductionRankedOperationV1 as OpR, ProductionRankedTerminatorV1 as EndR,
    ProductionRankedValueIdV1 as IdR, ProductionRankedValueV1 as ValueR,
};

pub(super) const STORAGE: usize = 1024 * 1024;
const WORK: usize = 131_072;
const OUTPUT: IdR = IdR::new(0);
const KERNARG32: IdR = IdR::new(1);
const KERNARG64: IdR = IdR::new(2);
const OPERATIONS: usize = 512;

fn invalid(message: &'static str) -> PhysicalEntryAuxErrorV20 {
    PhysicalEntryAuxErrorV20::Relation(message)
}
fn vector<T>(count: usize) -> Result<Vec<T>, PhysicalEntryAuxErrorV20> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    if values.capacity() > count {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    Ok(values)
}
fn copy<T: Copy>(values: &[T]) -> Result<Vec<T>, PhysicalEntryAuxErrorV20> {
    let mut output = vector(values.len())?;
    output.extend_from_slice(values);
    Ok(output)
}
fn storage_bound() -> Result<usize, PhysicalEntryAuxErrorV20> {
    Ok(argument_sum_v1(&[
        std::mem::size_of::<Recipe>(),
        argument_product_v1(6, std::mem::size_of::<Block>())?,
        argument_product_v1(4 * OPERATIONS + 1, std::mem::size_of::<OpR>())?,
        // <=64 actual instructions, <=4 results + one explicit dependency row,
        // six inputs per row; four edges with <=131 physical merge values.
        argument_product_v1(64 * 5 * 6 + 4 * 131 + 80, std::mem::size_of::<ValueR>())?,
        4 * std::mem::size_of::<u64>(),
        128,
    ])?)
}
struct Values {
    rows: [Option<(ValueId, ValueR)>; 768],
    count: usize,
}
impl Values {
    fn new() -> Self {
        Self {
            rows: [None; 768],
            count: 0,
        }
    }
    fn insert(&mut self, source: ValueId, target: ValueR) -> Result<(), PhysicalEntryAuxErrorV20> {
        if self.count == self.rows.len()
            || self.rows[..self.count]
                .iter()
                .flatten()
                .any(|(id, _)| *id == source)
        {
            return Err(invalid(
                "physical ranked duplicate or excessive SSA definition",
            ));
        }
        self.rows[self.count] = Some((source, target));
        self.count += 1;
        Ok(())
    }
    fn get(&self, source: ValueId) -> Result<ValueR, PhysicalEntryAuxErrorV20> {
        self.rows[..self.count]
            .iter()
            .flatten()
            .find_map(|(id, value)| (*id == source).then_some(*value))
            .ok_or_else(|| invalid("physical ranked undefined actual SSA input"))
    }
}
struct Operations {
    rows: Vec<OpR>,
    next: u32,
}
impl Operations {
    fn new(next: u32) -> Result<Self, PhysicalEntryAuxErrorV20> {
        Ok(Self {
            rows: vector(OPERATIONS)?,
            next,
        })
    }
    fn id(&mut self) -> Result<IdR, PhysicalEntryAuxErrorV20> {
        let id = IdR::new(self.next);
        self.next = self
            .next
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        Ok(id)
    }
    fn push(&mut self, operation: OpR) -> Result<(), PhysicalEntryAuxErrorV20> {
        if self.rows.len() == OPERATIONS {
            return Err(invalid("physical ranked operation bound"));
        }
        self.rows.push(operation);
        Ok(())
    }
    fn constant(&mut self, value: u64) -> Result<ValueR, PhysicalEntryAuxErrorV20> {
        let result = self.id()?;
        self.push(OpR::IndexConstant { result, value })?;
        Ok(ValueR::Local(result))
    }
    fn unknown(&mut self) -> Result<ValueR, PhysicalEntryAuxErrorV20> {
        let result = self.id()?;
        self.push(OpR::IndexUnknown { result })?;
        Ok(ValueR::Local(result))
    }
    fn binary(
        &mut self,
        kind: Binary,
        lhs: ValueR,
        rhs: ValueR,
    ) -> Result<ValueR, PhysicalEntryAuxErrorV20> {
        let result = self.id()?;
        self.push(OpR::IndexBinary {
            result,
            kind,
            lhs,
            rhs,
        })?;
        Ok(ValueR::Local(result))
    }
    fn joined(&mut self, inputs: &[ValueR]) -> Result<ValueR, PhysicalEntryAuxErrorV20> {
        if inputs.is_empty() || inputs.len() > 6 {
            return Err(invalid("physical ranked dependency arity"));
        }
        let result = self.id()?;
        self.push(OpR::DeterministicJoin {
            result,
            dependencies: copy(inputs)?,
        })?;
        Ok(ValueR::Local(result))
    }
    fn cast(&mut self, source: ValueR) -> Result<ValueR, PhysicalEntryAuxErrorV20> {
        let result = self.id()?;
        self.push(OpR::IndexUnsignedCast {
            result,
            source,
            bit_width: 32,
        })?;
        Ok(ValueR::Local(result))
    }
    fn invocation(&mut self) -> Result<ValueR, PhysicalEntryAuxErrorV20> {
        let result = self.id()?;
        self.push(OpR::InvocationIndex {
            result,
            dimension: 0,
            launch_extent: 128,
        })?;
        Ok(ValueR::Local(result))
    }
}
fn edge(values: &Values, inputs: &[ValueId]) -> Result<Vec<ValueR>, PhysicalEntryAuxErrorV20> {
    if inputs.len() > 131 {
        return Err(invalid("physical ranked merge bound"));
    }
    let mut result = vector(inputs.len())?;
    for input in inputs {
        result.push(values.get(*input)?);
    }
    Ok(result)
}
fn setup(operations: &mut Operations, grid: u64) -> Result<(), PhysicalEntryAuxErrorV20> {
    operations.push(OpR::ExecutionLayout {
        grid_identity: grid,
        global_extents: [128, 1, 1],
        workgroup_extents: [64, 1, 1],
        subgroup_size: 64,
        full_physical_workgroups: true,
    })?;
    operations.push(OpR::ViewInSpace {
        result: OUTPUT,
        element_width: 32,
        writable: true,
        shape: copy(&[dialect_kernel::DYNAMIC_EXTENT])?,
        dynamic_extents: copy(&[ValueR::Argument(0)])?,
        allocation_origin: 1,
        noalias_class: 1,
        memory_space: dialect_kernel::MemorySpaceAttr::Global,
    })?;
    operations.push(OpR::OwnershipContract {
        view: ValueR::Local(OUTPUT),
        coverage: dialect_kernel::OwnershipCoverageAttr::ExactEffectDomain,
        partition: dialect_kernel::OwnershipPartitionAttr::DenseRectangles,
    })?;
    // Two typed read views of the SAME compiler ABI allocation, not two new
    // logical parameters. Their separate origin from output is conditional on
    // the retained ABI disjointness requirement, NOT an authenticated pointer fact.
    for (result, width, count) in [(KERNARG32, 32, 8), (KERNARG64, 64, 4)] {
        operations.push(OpR::ViewInSpace {
            result,
            element_width: width,
            writable: false,
            shape: copy(&[count])?,
            dynamic_extents: Vec::new(),
            allocation_origin: 2,
            noalias_class: 2,
            memory_space: dialect_kernel::MemorySpaceAttr::Global,
        })?;
    }
    Ok(())
}
fn entry_values(
    operation: &fe2o3_kernel_ir::Operation,
    values: &mut Values,
    operations: &mut Operations,
) -> Result<(), PhysicalEntryAuxErrorV20> {
    let [base_low, base_high, group, lane, exec] = operation.results.as_slice() else {
        return Err(invalid("physical ranked entry live-ins"));
    };
    values.insert(base_low.id, operations.unknown()?)?;
    values.insert(base_high.id, operations.unknown()?)?;
    let gid = operations.invocation()?;
    let width = operations.constant(64)?;
    values.insert(group.id, operations.binary(Binary::Divide, gid, width)?)?;
    values.insert(lane.id, operations.binary(Binary::Remainder, gid, width)?)?;
    values.insert(exec.id, operations.constant(u64::MAX)?)?;
    Ok(())
}
/// The public checked owner supplies actual source launch custody. The inner
/// constructor is private; unit-test raw layouts are explicitly inert fixtures.
pub(super) fn physical_entry_ranked_recipe_v20(
    owner: &VerifiedCanonicalKernelIrModuleV20,
    launch: &crate::ProductionSourceLaunchRosterV1,
    formal: &PhysicalEntryMemoryObligationsV20,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Recipe, PhysicalEntryAuxErrorV20> {
    let [root] = launch.roots() else {
        return Err(invalid("physical ranked source root roster"));
    };
    let layout = root.layout();
    if root.source_rank() != 1
        || root.source_launch().max_grid() != [2, 1, 1]
        || layout.global_extents() != [128, 1, 1]
        || layout.workgroup_extents() != [64, 1, 1]
        || layout.subgroup_size() != 64
        || !layout.full_physical_workgroups()
    {
        return Err(invalid("physical ranked actual source geometry differs"));
    }
    recipe(owner, formal, layout.grid_identity(), budget)
}
fn recipe(
    owner: &VerifiedCanonicalKernelIrModuleV20,
    formal: &PhysicalEntryMemoryObligationsV20,
    grid: u64,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Recipe, PhysicalEntryAuxErrorV20> {
    budget.charge_work(WORK)?;
    if storage_bound()? > STORAGE {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.reserve_storage(STORAGE)?;
    let module = owner.module();
    let [function] = module.functions.as_slice() else {
        return Err(invalid("physical ranked function roster"));
    };
    let [kernel] = module.kernels.as_slice() else {
        return Err(invalid("physical ranked kernel roster"));
    };
    if kernel.entry != function.id
        || formal.canonical_identity() != owner.identity().digest()
        || formal.output().kernel() != &kernel.id
        || formal.output().entry() != &function.id
        || formal.kernarg_abi().minimum_bytes() != 32
        || formal.kernarg_abi().alignment() != 8
        || !formal.kernarg_abi().requires_immutable_kernarg()
        || formal.kernarg_abi().disjoint_output() != formal.store().allocation()
        || formal.output().bounds_requirements().len() != 1
        || formal.output().bounds_requirements()[0].minimum_byte_len() != Some(512)
    {
        return Err(invalid(
            "physical ranked combined memory subject or ABI obligation differs",
        ));
    }
    let body = function
        .body
        .as_ref()
        .ok_or_else(|| invalid("physical ranked body absent"))?;
    if !matches!(body.blocks.len(), 1 | 4) || body.parameters.len() != 5 {
        return Err(invalid("physical ranked exact body shape"));
    }
    let mut values = Values::new();
    for (index, value) in body.parameters.iter().enumerate().skip(1) {
        values.insert(*value, ValueR::Argument(index as u32))?;
    }
    for (index, block) in body.blocks.iter().enumerate() {
        if block.id.0 != index as u32 || block.parameters.len() > 131 {
            return Err(invalid("physical ranked actual block identity"));
        }
        for (argument, value) in block.parameters.iter().enumerate() {
            values.insert(
                value.id,
                ValueR::BlockArgument {
                    block: index as u32,
                    argument: argument as u32,
                },
            )?;
        }
    }
    let mut blocks = vector(body.blocks.len() + 2)?;
    let mut next = 3;
    let mut read_index = 0;
    let mut store_index = None;
    let mut steps = 0;
    for (block_index, block) in body.blocks.iter().enumerate() {
        let mut operations = Operations::new(next)?;
        if block_index == 0 {
            setup(&mut operations, grid)?;
        }
        let mut comparison = None;
        for (operation_index, operation) in block.operations.iter().enumerate() {
            let location =
                fe2o3_kernel_ir::FunctionOperationLocation::new(block.id, operation_index);
            let step = match operation.kind {
                Op::Gfx942PhysicalEntryDeclaration(_)
                    if block_index == 0 && operation_index == 0 =>
                {
                    entry_values(operation, &mut values, &mut operations)?;
                    continue;
                }
                Op::Gfx942PhysicalEntryStep(step) => step,
                _ => return Err(invalid("physical ranked unexpected executable operation")),
            };
            steps += 1;
            if steps > 64 {
                return Err(invalid("physical ranked native operation bound"));
            }
            let mut inputs = [None; 6];
            let mut input_count = 0;
            for input in step.operands.iter().flatten() {
                inputs[input_count] = Some(values.get(*input)?);
                input_count += 1;
            }
            // All actual SSA dependencies survive even when the analysis has a
            // more precise derived scalar/index summary. This row is not a
            // theorem about arithmetic or numerical pointer words.
            let mut dependencies = [ValueR::Argument(0); 6];
            for (index, input) in inputs[..input_count].iter().enumerate() {
                dependencies[index] = input.ok_or_else(|| invalid("physical ranked input hole"))?;
            }
            let dependencies = &dependencies[..input_count];
            if !dependencies.is_empty() {
                let _ = operations.joined(dependencies)?;
            }
            let opcode = step.instruction.opcode;
            let mut special = [None; 4];
            match opcode {
                Opcode::LoadKernargPair | Opcode::LoadKernargDword => {
                    let read = formal
                        .kernarg_reads()
                        .get(read_index)
                        .ok_or_else(|| invalid("physical ranked missing actual kernarg read"))?;
                    read_index += 1;
                    if read.location() != location
                        || read.source_site() != step.site
                        || read.byte_offset() != step.instruction.immediate
                        || read.base().map(Some) != [step.operands[0], step.operands[1]]
                        || read
                            .results()
                            .iter()
                            .flatten()
                            .copied()
                            .ne(operation.results.iter().map(|r| r.id))
                    {
                        return Err(invalid(
                            "physical ranked kernarg operation/SSA join differs",
                        ));
                    }
                    let width = read.byte_width();
                    let view = match width {
                        4 => KERNARG32,
                        8 => KERNARG64,
                        _ => return Err(invalid("physical ranked kernarg width")),
                    };
                    let index = operations.constant(u64::from(read.byte_offset() / width))?;
                    operations.push(OpR::Access {
                        kind: Access::Read,
                        view: ValueR::Local(view),
                        indices: copy(&[index])?,
                    })?;
                    match read.slot() {
                        Slot::OutputPointer => {}
                        Slot::OutputLength => {
                            let modulus = operations.constant(1u64 << 32)?;
                            special[0] = Some(operations.binary(
                                Binary::Remainder,
                                ValueR::Argument(0),
                                modulus,
                            )?);
                            special[1] = Some(operations.binary(
                                Binary::Divide,
                                ValueR::Argument(0),
                                modulus,
                            )?);
                        }
                        Slot::ScalarArgument(index) if (1..=4).contains(&index) => {
                            special[0] = Some(operations.cast(ValueR::Argument(u32::from(index)))?);
                        }
                        Slot::ScalarArgument(_) => {
                            return Err(invalid("physical ranked scalar ABI slot"));
                        }
                    }
                }
                Opcode::ScalarLshl32 => {
                    let multiplier = operations.constant(64)?;
                    special[0] =
                        Some(operations.binary(Binary::Multiply, dependencies[0], multiplier)?);
                }
                Opcode::VectorAddU32 => {
                    // The immutable physical verifier proved this exact current
                    // group*64 + lane SSA, under the exact bounded source launch.
                    special[0] = Some(operations.invocation()?);
                }
                Opcode::VectorMove32 if step.instruction.source0 == 255 => {
                    special[0] = Some(operations.constant(0)?);
                }
                Opcode::ScalarCompareEqZero => {
                    if comparison
                        .replace((dependencies[0], operations.constant(0)?))
                        .is_some()
                    {
                        return Err(invalid("physical ranked repeated selector comparison"));
                    }
                }
                Opcode::GlobalStoreDword => {
                    let store = formal.store();
                    if store.location() != location
                        || store.source_site() != step.site
                        || store.output() != body.parameters[0]
                        || [
                            Some(store.address()[0]),
                            Some(store.address()[1]),
                            Some(store.value()),
                            Some(store.exec()),
                            None,
                            None,
                        ] != step.operands
                    {
                        return Err(invalid("physical ranked actual output store join differs"));
                    }
                    // The formal attribution retains the exact compare/save
                    // producers and the physical owner proves their address,
                    // length, index and EXEC generation relationship.
                    let index = values.get(store.index()[0])?;
                    let _data = values.get(store.value())?;
                    if store_index.replace(index).is_some() {
                        return Err(invalid("physical ranked duplicate output effect"));
                    }
                }
                Opcode::WaitLgkm0
                | Opcode::VectorMove32
                | Opcode::VectorLshlrev64
                | Opcode::VectorAddCarry
                | Opcode::VectorAddCarryIn
                | Opcode::VectorCompareGtU64
                | Opcode::SaveAndMaskExec
                | Opcode::WaitVm0
                | Opcode::RestoreExec => {}
                Opcode::BranchScc1 | Opcode::Branch | Opcode::Endpgm0 | Opcode::Fallthrough => {
                    return Err(invalid("physical ranked control belongs to actual CFG"));
                }
            }
            for (index, result) in operation.results.iter().enumerate() {
                let target = match special.get(index).copied().flatten() {
                    Some(value) => value,
                    None => operations.joined(dependencies)?,
                };
                values.insert(result.id, target)?;
            }
        }
        let end = match block.terminator.as_ref() {
            Some(End::ConditionalBranch {
                then_target,
                then_arguments,
                else_target,
                else_arguments,
                ..
            }) => {
                let (lhs, rhs) = comparison
                    .ok_or_else(|| invalid("physical ranked actual selector comparison missing"))?;
                EndR::IndexEqualArgs {
                    lhs,
                    rhs,
                    true_arguments: edge(&values, then_arguments)?,
                    false_arguments: edge(&values, else_arguments)?,
                    true_block: then_target.0,
                    false_block: else_target.0,
                }
            }
            Some(End::Branch { target, arguments }) => EndR::BranchArgs {
                target: target.0,
                arguments: edge(&values, arguments)?,
            },
            Some(End::Return { values: returned }) if returned.is_empty() => {
                let index =
                    store_index.ok_or_else(|| invalid("physical ranked authored store absent"))?;
                let length = operations.id()?;
                operations.push(OpR::Dimension {
                    result: length,
                    view: ValueR::Local(OUTPUT),
                    dimension: 0,
                })?;
                EndR::IndexLessThanArgs {
                    lhs: index,
                    rhs: ValueR::Local(length),
                    true_arguments: copy(&[index])?,
                    false_arguments: Vec::new(),
                    true_block: body.blocks.len() as u32,
                    false_block: body.blocks.len() as u32 + 1,
                }
            }
            _ => return Err(invalid("physical ranked unexpected authored terminator")),
        };
        next = operations.next;
        blocks.push(Block::with_index_arguments(
            block.parameters.len() as u32,
            operations.rows,
            end,
        ));
    }
    if read_index != formal.kernarg_reads().len() || store_index.is_none() {
        return Err(invalid(
            "physical ranked complete memory occurrence census differs",
        ));
    }
    let store_block = body.blocks.len() as u32;
    let mut store = vector(1)?;
    store.push(OpR::Access {
        kind: Access::Write,
        view: ValueR::Local(OUTPUT),
        indices: copy(&[ValueR::BlockArgument {
            block: store_block,
            argument: 0,
        }])?,
    });
    blocks.push(Block::with_index_arguments(
        1,
        store,
        EndR::Branch {
            target: store_block + 1,
        },
    ));
    blocks.push(Block::new(Vec::new(), EndR::Return));
    Recipe::new(function.id.as_str(), 5, blocks).map_err(PhysicalEntryAuxErrorV20::RankedRecipe)
}
#[cfg(test)]
#[path = "production_physical_entry_ranked_v20_tests.rs"]
mod tests;
