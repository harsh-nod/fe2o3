//! Fixed conditional safety projection, not executable code or functional proof.
//! The full-EXEC input read is never moved under an invented length branch.
//! Its 128-element prefix and noalias classes are conditional on the complete
//! typed runtime report retained beside this recipe by the checked source owner.
use super::*;
use dialect_kernel::{AccessKindAttr as Access, IndexBinaryKindAttr as Binary};
use fe2o3_kernel_ir::{
    Gfx942PhysicalGlobalCopyOpcodeV1 as Opcode, OperationKind as Op,
    PhysicalGlobalCopyKernargSlotV21 as Slot, PhysicalGlobalCopyMemoryObligationsV21,
    Terminator as End, VerifiedCanonicalKernelIrModuleV21,
};
use fe2o3_pliron::{
    ProductionRankedBlockV1 as Block, ProductionRankedKernelV1 as Recipe,
    ProductionRankedOperationV1 as OpR, ProductionRankedTerminatorV1 as EndR,
    ProductionRankedValueIdV1 as IdR, ProductionRankedValueV1 as ValueR,
};
pub(super) const STORAGE: usize = 1024 * 1024;
const WORK: usize = 131_072;
const INPUT_PREFIX: IdR = IdR::new(0);
const OUTPUT: IdR = IdR::new(1);
const KERNARG64: IdR = IdR::new(2);
const OPERATIONS: usize = 256;
fn invalid(s: &'static str) -> PhysicalGlobalCopyAuxErrorV21 {
    PhysicalGlobalCopyAuxErrorV21::Relation(s)
}
fn vector<T>(count: usize) -> Result<Vec<T>, PhysicalGlobalCopyAuxErrorV21> {
    let mut v = Vec::new();
    v.try_reserve_exact(count)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    if v.capacity() > count {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    Ok(v)
}
fn copy<T: Copy>(values: &[T]) -> Result<Vec<T>, PhysicalGlobalCopyAuxErrorV21> {
    let mut v = vector(values.len())?;
    v.extend_from_slice(values);
    Ok(v)
}
fn storage_bound() -> Result<usize, PhysicalGlobalCopyAuxErrorV21> {
    Ok(argument_sum_v1(&[
        std::mem::size_of::<Recipe>(),
        3 * std::mem::size_of::<Block>(),
        (OPERATIONS + 1) * std::mem::size_of::<OpR>(),
        (32 * 5 * 5 + 64) * std::mem::size_of::<ValueR>(),
        4 * std::mem::size_of::<u64>(),
        128,
    ])?)
}
struct Values {
    rows: [Option<(ValueId, ValueR)>; 256],
    count: usize,
}
impl Values {
    fn new() -> Self {
        Self {
            rows: [None; 256],
            count: 0,
        }
    }
    fn insert(
        &mut self,
        source: ValueId,
        target: ValueR,
    ) -> Result<(), PhysicalGlobalCopyAuxErrorV21> {
        if self.count == self.rows.len()
            || self.rows[..self.count]
                .iter()
                .flatten()
                .any(|(id, _)| *id == source)
        {
            return Err(invalid(
                "global copy ranked duplicate or excessive SSA definition",
            ));
        }
        self.rows[self.count] = Some((source, target));
        self.count += 1;
        Ok(())
    }
    fn get(&self, source: ValueId) -> Result<ValueR, PhysicalGlobalCopyAuxErrorV21> {
        self.rows[..self.count]
            .iter()
            .flatten()
            .find_map(|(id, value)| (*id == source).then_some(*value))
            .ok_or_else(|| invalid("global copy ranked undefined actual SSA input"))
    }
}
struct Operations {
    rows: Vec<OpR>,
    next: u32,
}
impl Operations {
    fn new() -> Result<Self, PhysicalGlobalCopyAuxErrorV21> {
        Ok(Self {
            rows: vector(OPERATIONS)?,
            next: 3,
        })
    }
    fn id(&mut self) -> Result<IdR, PhysicalGlobalCopyAuxErrorV21> {
        let id = IdR::new(self.next);
        self.next = self
            .next
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        Ok(id)
    }
    fn push(&mut self, op: OpR) -> Result<(), PhysicalGlobalCopyAuxErrorV21> {
        if self.rows.len() == OPERATIONS {
            return Err(invalid("global copy ranked operation bound"));
        }
        self.rows.push(op);
        Ok(())
    }
    fn constant(&mut self, value: u64) -> Result<ValueR, PhysicalGlobalCopyAuxErrorV21> {
        let result = self.id()?;
        self.push(OpR::IndexConstant { result, value })?;
        Ok(ValueR::Local(result))
    }
    fn unknown(&mut self) -> Result<ValueR, PhysicalGlobalCopyAuxErrorV21> {
        let result = self.id()?;
        self.push(OpR::IndexUnknown { result })?;
        Ok(ValueR::Local(result))
    }
    fn binary(
        &mut self,
        kind: Binary,
        lhs: ValueR,
        rhs: ValueR,
    ) -> Result<ValueR, PhysicalGlobalCopyAuxErrorV21> {
        let result = self.id()?;
        self.push(OpR::IndexBinary {
            result,
            kind,
            lhs,
            rhs,
        })?;
        Ok(ValueR::Local(result))
    }
    fn joined(&mut self, inputs: &[ValueR]) -> Result<ValueR, PhysicalGlobalCopyAuxErrorV21> {
        if inputs.is_empty() || inputs.len() > 5 {
            return Err(invalid("global copy ranked dependency arity"));
        }
        let result = self.id()?;
        self.push(OpR::DeterministicJoin {
            result,
            dependencies: copy(inputs)?,
        })?;
        Ok(ValueR::Local(result))
    }
    fn invocation(&mut self) -> Result<ValueR, PhysicalGlobalCopyAuxErrorV21> {
        let result = self.id()?;
        self.push(OpR::InvocationIndex {
            result,
            dimension: 0,
            launch_extent: 128,
        })?;
        Ok(ValueR::Local(result))
    }
}
pub(super) fn required_conditions(report: &PhysicalGlobalCopyMemoryObligationsV21) -> bool {
    let runtime = report.runtime_requirements();
    let abi = report.kernarg_abi();
    let global = report.global();
    let read = report.input_read().access();
    let store = report.output_store().access();
    runtime.input() == read.allocation()
        && runtime.output() == store.allocation()
        && runtime.input().parameter_index() == 0
        && runtime.output().parameter_index() == 1
        && runtime.minimum_input_bytes() == 512
        && runtime.minimum_output_bytes() == 512
        && runtime.requires_input_readable()
        && runtime.requires_input_initialized()
        && runtime.requires_output_writable()
        && runtime.requires_input_output_disjoint()
        && abi.minimum_bytes() == 32
        && abi.alignment() == 8
        && abi.disjoint_output() == store.allocation()
        && abi.requires_readable_kernarg()
        && abi.requires_live_kernarg()
        && abi.requires_immutable_kernarg()
        && global.allocations().len() == 2
        && global.accesses().len() == 2
        && global.bounds_requirements().len() == 2
        && global
            .bounds_requirements()
            .iter()
            .all(|b| b.minimum_byte_len() == Some(512))
        && global.runtime_alias_requirements().len() == 1
        && global.runtime_alias_requirements()[0].left() == runtime.input()
        && global.runtime_alias_requirements()[0].right() == runtime.output()
        && [
            global.runtime_alias_requirements()[0].left_accessed_bytes(),
            global.runtime_alias_requirements()[0].right_accessed_bytes(),
        ]
        .iter()
        .all(|r| r.is_some_and(|r| r.start() == 0 && r.end_exclusive() == 512))
        && global.inter_invocation_conflicts().is_empty()
}
fn setup(ops: &mut Operations, grid: u64) -> Result<(), PhysicalGlobalCopyAuxErrorV21> {
    ops.push(OpR::ExecutionLayout {
        grid_identity: grid,
        global_extents: [128, 1, 1],
        workgroup_extents: [64, 1, 1],
        subgroup_size: 64,
        full_physical_workgroups: true,
    })?;
    // A conditional READABLE PREFIX of the dynamic source input, NOT an
    // assertion that its real length is128. The entire report remains required.
    ops.push(OpR::ViewInSpace {
        result: INPUT_PREFIX,
        element_width: 32,
        writable: false,
        shape: copy(&[128])?,
        dynamic_extents: Vec::new(),
        allocation_origin: 1,
        noalias_class: 1,
        memory_space: dialect_kernel::MemorySpaceAttr::Global,
    })?;
    ops.push(OpR::ViewInSpace {
        result: OUTPUT,
        element_width: 32,
        writable: true,
        shape: copy(&[dialect_kernel::DYNAMIC_EXTENT])?,
        dynamic_extents: copy(&[ValueR::Argument(1)])?,
        allocation_origin: 2,
        noalias_class: 2,
        memory_space: dialect_kernel::MemorySpaceAttr::Global,
    })?;
    ops.push(OpR::OwnershipContract {
        view: ValueR::Local(OUTPUT),
        coverage: dialect_kernel::OwnershipCoverageAttr::ExactEffectDomain,
        partition: dialect_kernel::OwnershipPartitionAttr::DenseRectangles,
    })?;
    // Compiler ABI view, not a third source argument. Only output-versus-ABI
    // disjointness is required; overlap between two readonly views is harmless.
    ops.push(OpR::ViewInSpace {
        result: KERNARG64,
        element_width: 64,
        writable: false,
        shape: copy(&[4])?,
        dynamic_extents: Vec::new(),
        allocation_origin: 3,
        noalias_class: 3,
        memory_space: dialect_kernel::MemorySpaceAttr::Global,
    })?;
    Ok(())
}
fn entry_values(
    op: &fe2o3_kernel_ir::Operation,
    values: &mut Values,
    ops: &mut Operations,
) -> Result<(), PhysicalGlobalCopyAuxErrorV21> {
    let [base_low, base_high, group, lane, exec] = op.results.as_slice() else {
        return Err(invalid("global copy ranked entry live-ins"));
    };
    values.insert(base_low.id, ops.unknown()?)?;
    values.insert(base_high.id, ops.unknown()?)?;
    let gid = ops.invocation()?;
    let width = ops.constant(64)?;
    values.insert(group.id, ops.binary(Binary::Divide, gid, width)?)?;
    values.insert(lane.id, ops.binary(Binary::Remainder, gid, width)?)?;
    values.insert(exec.id, ops.constant(u64::MAX)?)?;
    Ok(())
}
pub(super) fn physical_global_copy_ranked_recipe_v21(
    owner: &VerifiedCanonicalKernelIrModuleV21,
    launch: &crate::ProductionSourceLaunchRosterV1,
    formal: &PhysicalGlobalCopyMemoryObligationsV21,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Recipe, PhysicalGlobalCopyAuxErrorV21> {
    let [root] = launch.roots() else {
        return Err(invalid("global copy ranked source root roster"));
    };
    let layout = root.layout();
    if root.source_rank() != 1
        || root.source_launch().max_grid() != [2, 1, 1]
        || layout.global_extents() != [128, 1, 1]
        || layout.workgroup_extents() != [64, 1, 1]
        || layout.subgroup_size() != 64
        || !layout.full_physical_workgroups()
    {
        return Err(invalid("global copy ranked actual source geometry differs"));
    }
    recipe(owner, formal, layout.grid_identity(), budget)
}
fn recipe(
    owner: &VerifiedCanonicalKernelIrModuleV21,
    formal: &PhysicalGlobalCopyMemoryObligationsV21,
    grid: u64,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Recipe, PhysicalGlobalCopyAuxErrorV21> {
    budget.charge_work(WORK)?;
    if storage_bound()? > STORAGE {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.reserve_storage(STORAGE)?;
    let [function] = owner.module().functions.as_slice() else {
        return Err(invalid("global copy ranked one function"));
    };
    let [kernel] = owner.module().kernels.as_slice() else {
        return Err(invalid("global copy ranked one kernel"));
    };
    if kernel.entry != function.id
        || formal.canonical_identity() != owner.identity().digest()
        || formal.global().kernel() != &kernel.id
        || formal.global().entry() != &function.id
        || !required_conditions(formal)
    {
        return Err(invalid(
            "global copy ranked complete memory subject or conditions differ",
        ));
    }
    let body = function
        .body
        .as_ref()
        .ok_or_else(|| invalid("global copy ranked body missing"))?;
    let [block] = body.blocks.as_slice() else {
        return Err(invalid("global copy ranked one actual block"));
    };
    if body.parameters.len() != 2 || !block.parameters.is_empty() {
        return Err(invalid("global copy ranked exact real parameters"));
    }
    let mut values = Values::new();
    let mut ops = Operations::new()?;
    setup(&mut ops, grid)?;
    let mut abi_cursor = 0;
    let mut read_seen = false;
    let mut store_index = None;
    for (operation_index, operation) in block.operations.iter().enumerate() {
        let location = fe2o3_kernel_ir::FunctionOperationLocation::new(block.id, operation_index);
        let step = match operation.kind {
            Op::Gfx942PhysicalGlobalCopyDeclaration(_) if operation_index == 0 => {
                entry_values(operation, &mut values, &mut ops)?;
                continue;
            }
            Op::Gfx942PhysicalGlobalCopyStep(step) => step,
            _ => {
                return Err(invalid(
                    "global copy ranked unexpected executable operation",
                ));
            }
        };
        if operation_index > 32 {
            return Err(invalid("global copy ranked native operation bound"));
        }
        let mut dependencies = [ValueR::Argument(0); 5];
        let mut input_count = 0;
        for input in step.operands.iter().flatten() {
            dependencies[input_count] = values.get(*input)?;
            input_count += 1;
        }
        let dependencies = &dependencies[..input_count];
        if !dependencies.is_empty() {
            let _ = ops.joined(dependencies)?;
        }
        let mut special = [None; 4];
        match step.instruction.opcode {
            Opcode::LoadKernargPair => {
                let read = formal
                    .kernarg_reads()
                    .get(abi_cursor)
                    .ok_or_else(|| invalid("global copy ranked missing kernarg row"))?;
                abi_cursor += 1;
                if read.location() != location
                    || read.source_site() != step.site
                    || read.byte_offset() != step.instruction.immediate
                    || read.base().map(Some) != [step.operands[0], step.operands[1]]
                    || read
                        .results()
                        .into_iter()
                        .ne(operation.results.iter().map(|r| r.id))
                {
                    return Err(invalid(
                        "global copy ranked exact kernarg operation SSA join",
                    ));
                }
                let index = ops.constant(u64::from(read.byte_offset() / 8))?;
                ops.push(OpR::Access {
                    kind: Access::Read,
                    view: ValueR::Local(KERNARG64),
                    indices: copy(&[index])?,
                })?;
                match read.slot() {
                    Slot::InputPointer | Slot::OutputPointer => {}
                    Slot::InputLength | Slot::OutputLength => {
                        let argument = ValueR::Argument(if read.slot() == Slot::InputLength {
                            0
                        } else {
                            1
                        });
                        let modulus = ops.constant(1u64 << 32)?;
                        special[0] = Some(ops.binary(Binary::Remainder, argument, modulus)?);
                        special[1] = Some(ops.binary(Binary::Divide, argument, modulus)?);
                    }
                }
            }
            Opcode::ScalarLshl32 => {
                let multiplier = ops.constant(64)?;
                special[0] = Some(ops.binary(Binary::Multiply, dependencies[0], multiplier)?);
            }
            Opcode::VectorAddU32 => special[0] = Some(ops.invocation()?),
            Opcode::VectorMove32 if step.instruction.source0 == 255 => {
                special[0] = Some(ops.constant(0)?)
            }
            Opcode::GlobalLoadDword => {
                let read = formal.input_read();
                let access = read.access();
                if read_seen
                    || access.location() != location
                    || access.source_site() != step.site
                    || access.parameter() != body.parameters[0]
                    || operation.results.first().map(|r| r.id) != Some(read.result())
                    || [
                        Some(access.address()[0]),
                        Some(access.address()[1]),
                        Some(access.exec()),
                        None,
                        None,
                    ] != step.operands
                {
                    return Err(invalid("global copy ranked actual full input read join"));
                }
                let index = values.get(access.index()[0])?;
                ops.push(OpR::Access {
                    kind: Access::Read,
                    view: ValueR::Local(INPUT_PREFIX),
                    indices: copy(&[index])?,
                })?;
                // Read data is NOT DeterministicJoin(address,EXEC). Address
                // dependencies remain in their separate row above.
                special[0] = Some(ops.unknown()?);
                read_seen = true;
            }
            Opcode::GlobalStoreDword => {
                let store = formal.output_store();
                let access = store.access();
                if !read_seen
                    || access.location() != location
                    || access.source_site() != step.site
                    || access.parameter() != body.parameters[1]
                    || store.value() != formal.input_read().result()
                    || [
                        Some(access.address()[0]),
                        Some(access.address()[1]),
                        Some(store.value()),
                        Some(access.exec()),
                        None,
                    ] != step.operands
                {
                    return Err(invalid(
                        "global copy ranked actual ready input to output join",
                    ));
                }
                let index = values.get(access.index()[0])?;
                let _data = values.get(store.value())?;
                if store_index.replace(index).is_some() {
                    return Err(invalid("global copy ranked duplicate output effect"));
                }
            }
            Opcode::WaitLgkm0
            | Opcode::WaitVm0
            | Opcode::VectorMove32
            | Opcode::VectorLshlrev64
            | Opcode::VectorAddCarry
            | Opcode::VectorAddCarryIn
            | Opcode::VectorCompareGtU64
            | Opcode::SaveAndMaskExec
            | Opcode::RestoreExec => {}
            Opcode::Endpgm0 => return Err(invalid("global copy ranked end belongs to actual CFG")),
        }
        for (index, result) in operation.results.iter().enumerate() {
            let value = match special.get(index).copied().flatten() {
                Some(v) => v,
                None => ops.joined(dependencies)?,
            };
            values.insert(result.id, value)?;
        }
    }
    if abi_cursor != 4 || !read_seen {
        return Err(invalid("global copy ranked complete read census"));
    }
    let index = store_index.ok_or_else(|| invalid("global copy ranked actual output absent"))?;
    if !matches!(block.terminator.as_ref(),Some(End::Return{values}) if values.is_empty()) {
        return Err(invalid("global copy ranked actual return differs"));
    }
    let length = ops.id()?;
    ops.push(OpR::Dimension {
        result: length,
        view: ValueR::Local(OUTPUT),
        dimension: 0,
    })?;
    let end = EndR::IndexLessThanArgs {
        lhs: index,
        rhs: ValueR::Local(length),
        true_arguments: copy(&[index])?,
        false_arguments: Vec::new(),
        true_block: 1,
        false_block: 2,
    };
    let mut blocks = vector(3)?;
    blocks.push(Block::new(ops.rows, end));
    let mut store = vector(1)?;
    store.push(OpR::Access {
        kind: Access::Write,
        view: ValueR::Local(OUTPUT),
        indices: copy(&[ValueR::BlockArgument {
            block: 1,
            argument: 0,
        }])?,
    });
    blocks.push(Block::with_index_arguments(
        1,
        store,
        EndR::Branch { target: 2 },
    ));
    blocks.push(Block::new(Vec::new(), EndR::Return));
    Recipe::new(function.id.as_str(), 2, blocks)
        .map_err(PhysicalGlobalCopyAuxErrorV21::RankedRecipe)
}
#[cfg(test)]
#[path = "production_physical_global_copy_ranked_v21_tests.rs"]
mod tests;
