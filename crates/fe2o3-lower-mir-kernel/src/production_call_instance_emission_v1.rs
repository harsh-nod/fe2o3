// This is emission, not verification or a source-correspondence receipt.
// Input owner charges stay live; additional_storage_bytes belongs to the output.
#[derive(Debug, Eq, PartialEq)]
enum CallInstanceEmissionErrorV1 {
    Resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1),
    MissingBody,
    InvalidRole,
    MissingCall,
    WrongCallee,
    SignatureMismatch,
    MissingDefinition,
    DuplicateIdentity,
    IdentityRangeOverlap,
    InvalidContinuation,
    MissingTerminator,
    ForeignBlock,
    ExecutionTransport,
    CalleeFrameAllocation,
    CalleeWorkgroupAllocation,
    CalleeCollective,
    CalleeOrderedContract,
    CalleeInlineAssembly,
}

impl From<fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1>
    for CallInstanceEmissionErrorV1
{
    fn from(error: fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1) -> Self {
        Self::Resource(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CallInstanceSplitV1 {
    call: FunctionOperationLocation,
    entry: BlockId,
    callee_entry: BlockId,
    continuation: BlockId,
    callee_blocks: usize,
    returns: usize,
    result_components: usize,
}

#[derive(Debug)]
struct SplicedCallInstanceV1 {
    caller: Function,
    // The orchestrator must retain these requirements when composing the module.
    callee_required_capabilities: BTreeSet<fe2o3_kernel_ir::TargetCapability>,
    split: CallInstanceSplitV1,
    additional_storage_bytes: usize,
}

fn call_splice_arithmetic_v1() -> CallInstanceEmissionErrorV1 {
    fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic.into()
}

fn call_splice_charge_storage_v1(
    bytes: usize,
    budget: &mut ArgumentBudgetV1<'_>,
    owned: &mut usize,
) -> Result<(), CallInstanceEmissionErrorV1> {
    let next = owned
        .checked_add(bytes)
        .ok_or_else(call_splice_arithmetic_v1)?;
    budget.reserve_storage(bytes)?;
    *owned = next;
    Ok(())
}

fn call_splice_vec_v1<T>(
    count: usize,
    budget: &mut ArgumentBudgetV1<'_>,
    owned: &mut usize,
) -> Result<Vec<T>, CallInstanceEmissionErrorV1> {
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(call_splice_arithmetic_v1)?;
    call_splice_charge_storage_v1(bytes, budget, owned)?;
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| {
        CallInstanceEmissionErrorV1::Resource(
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Allocation,
        )
    })?;
    Ok(values)
}

fn call_splice_search_work_v1(length: usize) -> usize {
    (usize::BITS - length.leading_zeros()) as usize + 1
}

fn call_splice_sort_work_v1(
    length: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), CallInstanceEmissionErrorV1> {
    let work = length
        .checked_mul(call_splice_search_work_v1(length))
        .and_then(|work| work.checked_mul(4))
        .ok_or_else(call_splice_arithmetic_v1)?;
    budget.charge_work(work)?;
    Ok(())
}

fn call_splice_type_eq_v1(
    mut left: &Type,
    mut right: &Type,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, CallInstanceEmissionErrorV1> {
    loop {
        budget.charge_work(1)?;
        match (left, right) {
            (Type::Execution(_), _) | (_, Type::Execution(_)) => {
                return Err(CallInstanceEmissionErrorV1::ExecutionTransport);
            }
            (Type::Pointer(a), Type::Pointer(b))
                if a.address_space == b.address_space && a.access == b.access =>
            {
                left = &a.pointee;
                right = &b.pointee;
            }
            (Type::Slice(a), Type::Slice(b))
                if a.address_space == b.address_space && a.access == b.access =>
            {
                left = &a.element;
                right = &b.element;
            }
            (Type::Unit, Type::Unit) => return Ok(true),
            (Type::Scalar(a), Type::Scalar(b)) => return Ok(a == b),
            (Type::Vector(a), Type::Vector(b)) => return Ok(a == b),
            _ => return Ok(false),
        }
    }
}

struct CallSpliceIndexV1<'a> {
    blocks: Vec<BlockId>,
    values: Vec<(ValueId, &'a Type)>,
}

impl<'a> CallSpliceIndexV1<'a> {
    fn value(
        &self,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'a Type, CallInstanceEmissionErrorV1> {
        budget.charge_work(call_splice_search_work_v1(self.values.len()))?;
        self.values
            .binary_search_by_key(&value, |(id, _)| *id)
            .map(|index| self.values[index].1)
            .map_err(|_| CallInstanceEmissionErrorV1::MissingDefinition)
    }

    fn target(
        &self,
        target: BlockId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), CallInstanceEmissionErrorV1> {
        budget.charge_work(call_splice_search_work_v1(self.blocks.len()))?;
        if self.blocks.binary_search(&target).is_err() {
            return Err(CallInstanceEmissionErrorV1::ForeignBlock);
        }
        Ok(())
    }
}

fn call_splice_index_v1<'a>(
    function: &'a Function,
    budget: &mut ArgumentBudgetV1<'_>,
    scratch: &mut usize,
) -> Result<CallSpliceIndexV1<'a>, CallInstanceEmissionErrorV1> {
    let body = function
        .body
        .as_ref()
        .ok_or(CallInstanceEmissionErrorV1::MissingBody)?;
    if body.blocks.is_empty() || body.parameters.len() != function.signature.parameters.len() {
        return Err(CallInstanceEmissionErrorV1::SignatureMismatch);
    }
    for ty in &function.signature.results {
        call_splice_type_eq_v1(ty, ty, budget)?;
    }
    let mut count = body.parameters.len();
    budget.charge_work(body.blocks.len())?;
    for block in &body.blocks {
        count = count
            .checked_add(block.parameters.len())
            .ok_or_else(call_splice_arithmetic_v1)?;
        budget.charge_work(block.operations.len())?;
        for operation in &block.operations {
            count = count
                .checked_add(operation.results.len())
                .ok_or_else(call_splice_arithmetic_v1)?;
        }
    }
    let mut blocks = call_splice_vec_v1(body.blocks.len(), budget, scratch)?;
    let mut values = call_splice_vec_v1(count, budget, scratch)?;
    budget.charge_work(count)?;
    values.extend(
        body.parameters
            .iter()
            .copied()
            .zip(&function.signature.parameters),
    );
    budget.charge_work(body.blocks.len())?;
    for block in &body.blocks {
        blocks.push(block.id);
        values.extend(block.parameters.iter().map(|value| (value.id, &value.ty)));
        budget.charge_work(block.operations.len())?;
        for operation in &block.operations {
            if matches!(operation.kind, OperationKind::Execution(_)) {
                return Err(CallInstanceEmissionErrorV1::ExecutionTransport);
            }
            values.extend(operation.results.iter().map(|value| (value.id, &value.ty)));
        }
    }
    call_splice_sort_work_v1(blocks.len(), budget)?;
    blocks.sort_unstable();
    call_splice_sort_work_v1(values.len(), budget)?;
    values.sort_unstable_by_key(|(id, _)| *id);
    budget.charge_work(
        blocks
            .len()
            .checked_add(values.len())
            .ok_or_else(call_splice_arithmetic_v1)?,
    )?;
    if blocks.windows(2).any(|rows| rows[0] == rows[1])
        || values.windows(2).any(|rows| rows[0].0 == rows[1].0)
    {
        return Err(CallInstanceEmissionErrorV1::DuplicateIdentity);
    }
    for (_, ty) in &values {
        call_splice_type_eq_v1(ty, ty, budget)?;
    }
    Ok(CallSpliceIndexV1 { blocks, values })
}

// Allocation lifetimes, declaration identity and collective participation need
// a checked source-instance relation before these callee occurrences can move.
fn call_splice_check_callee_operation_v1(
    kind: &OperationKind,
) -> Result<(), CallInstanceEmissionErrorV1> {
    use CallInstanceEmissionErrorV1 as Error;
    use OperationKind as Op;
    match kind {
        Op::Execution(_) => Err(Error::ExecutionTransport),
        Op::Alloca { .. } => Err(Error::CalleeFrameAllocation),
        Op::WorkgroupMemory(_) => Err(Error::CalleeWorkgroupAllocation),
        Op::Barrier(_)
        | Op::WorkgroupBarrier(_)
        | Op::Wave(_)
        | Op::Matrix(_)
        | Op::Gfx950LdsTranspose(_) => Err(Error::CalleeCollective),
        Op::VerificationContract(_) => Err(Error::CalleeOrderedContract),
        Op::InlineAssembly(_) | Op::Gfx942OrderedRegion(_) | Op::Gfx942OrderedProgram(_) => {
            Err(Error::CalleeInlineAssembly)
        }
        Op::Intrinsic(intrinsic) => match intrinsic.kind {
            fe2o3_kernel_ir::IntrinsicKind::InvocationIndex { .. }
            | fe2o3_kernel_ir::IntrinsicKind::LaunchExtent { .. } => Ok(()),
        },
        Op::Constant(_)
        | Op::MemoryIntrinsic(_)
        | Op::Unary { .. }
        | Op::Binary { .. }
        | Op::Compare { .. }
        | Op::Cast { .. }
        | Op::Select { .. }
        | Op::Call { .. }
        | Op::SliceLength { .. }
        | Op::SliceData { .. }
        | Op::GetElementPointer { .. }
        | Op::Load { .. }
        | Op::GuardedLoad { .. }
        | Op::GuardedStore { .. }
        | Op::Store { .. }
        | Op::Atomic(_)
        | Op::Fence(_)
        | Op::VectorLoad(_)
        | Op::VectorStore(_)
        | Op::VectorLayoutConvert(_) => Ok(()),
    }
}

#[cfg(test)]
#[test]
fn call_splice_preserves_direct_root_authoring_boundary_v1() {
    use fe2o3_kernel_ir::{
        AssemblySourceIdentity, Gfx942OrderedProgramRegistersV1, Gfx942OrderedProgramV1,
        Gfx942OrderedRegionRegistersV1, Gfx942OrderedRegionV1, Gfx942U32ProgramV1,
    };
    let source = AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]);
    let inputs = [ValueId(0), ValueId(1), ValueId(2)];
    let region = Gfx942OrderedRegionV1::new(
        source,
        Gfx942OrderedRegionRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
        inputs,
    )
    .unwrap();
    let mut descriptors = [0; 16];
    descriptors[0] = 8; // mov(out, input0)
    let program = Gfx942OrderedProgramV1::new(
        source,
        Gfx942OrderedProgramRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
        inputs,
        Gfx942U32ProgramV1::from_descriptors(1, descriptors).unwrap(),
    )
    .unwrap();
    for operation in [
        OperationKind::Gfx942OrderedRegion(region),
        OperationKind::Gfx942OrderedProgram(program),
    ] {
        assert_eq!(
            call_splice_check_callee_operation_v1(&operation),
            Err(CallInstanceEmissionErrorV1::CalleeInlineAssembly)
        );
    }
}

fn call_splice_check_body_v1(
    function: &Function,
    index: &CallSpliceIndexV1<'_>,
    callee: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, CallInstanceEmissionErrorV1> {
    let body = function
        .body
        .as_ref()
        .ok_or(CallInstanceEmissionErrorV1::MissingBody)?;
    if callee && !body.blocks[0].parameters.is_empty() {
        return Err(CallInstanceEmissionErrorV1::SignatureMismatch);
    }
    let mut returns = 0_usize;
    budget.charge_work(body.blocks.len())?;
    for block in &body.blocks {
        budget.charge_work(block.operations.len())?;
        for operation in &block.operations {
            if callee {
                call_splice_check_callee_operation_v1(&operation.kind)?;
            }
            if let OperationKind::InlineAssembly(assembly) = &operation.kind {
                budget.charge_work(assembly.operands.len())?;
            }
            operation
                .kind
                .try_visit_operands(|value| index.value(value, budget).map(|_| ()))?;
        }
        let terminator = block
            .terminator
            .as_ref()
            .ok_or(CallInstanceEmissionErrorV1::MissingTerminator)?;
        match terminator {
            Terminator::Switch { cases, .. } => budget.charge_work(cases.len())?,
            Terminator::IntegerSwitch { cases, .. } => budget.charge_work(cases.len())?,
            _ => {}
        }
        terminator.try_visit_operands(|value| index.value(value, budget).map(|_| ()))?;
        match terminator {
            Terminator::Branch { target, .. } => index.target(*target, budget)?,
            Terminator::ConditionalBranch {
                then_target,
                else_target,
                ..
            } => {
                index.target(*then_target, budget)?;
                index.target(*else_target, budget)?;
            }
            Terminator::Switch {
                cases,
                default_target,
                ..
            } => {
                for case in cases {
                    index.target(case.target, budget)?;
                }
                index.target(*default_target, budget)?;
            }
            Terminator::IntegerSwitch {
                cases,
                default_target,
                ..
            } => {
                for case in cases {
                    index.target(case.target, budget)?;
                }
                index.target(*default_target, budget)?;
            }
            Terminator::Return { values } => {
                if values.len() != function.signature.results.len() {
                    return Err(CallInstanceEmissionErrorV1::SignatureMismatch);
                }
                for (value, expected) in values.iter().zip(&function.signature.results) {
                    if !call_splice_type_eq_v1(index.value(*value, budget)?, expected, budget)? {
                        return Err(CallInstanceEmissionErrorV1::SignatureMismatch);
                    }
                }
                returns = returns
                    .checked_add(1)
                    .ok_or_else(call_splice_arithmetic_v1)?;
            }
            Terminator::Unreachable => {}
        }
    }
    Ok(returns)
}

fn call_splice_disjoint_v1<T: Ord>(
    left: impl ExactSizeIterator<Item = T>,
    right: impl ExactSizeIterator<Item = T>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, CallInstanceEmissionErrorV1> {
    budget.charge_work(
        left.len()
            .checked_add(right.len())
            .ok_or_else(call_splice_arithmetic_v1)?,
    )?;
    let mut left = left.peekable();
    let mut right = right.peekable();
    while let (Some(a), Some(b)) = (left.peek(), right.peek()) {
        match a.cmp(b) {
            std::cmp::Ordering::Less => {
                left.next();
            }
            std::cmp::Ordering::Greater => {
                right.next();
            }
            std::cmp::Ordering::Equal => return Ok(false),
        }
    }
    Ok(true)
}

fn splice_production_call_instance_v1(
    mut caller: Function,
    mut callee: Function,
    site: FunctionOperationLocation,
    entry: BlockId,
    continuation: BlockId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SplicedCallInstanceV1, CallInstanceEmissionErrorV1> {
    let mut scratch = 0_usize;
    let mut retained = 0_usize;
    let result = (|| {
        budget.charge_work(1)?;
        if callee.role != fe2o3_kernel_ir::FunctionRole::InternalHelper
            || caller.role == fe2o3_kernel_ir::FunctionRole::ExternalImport
        {
            return Err(CallInstanceEmissionErrorV1::InvalidRole);
        }
        let (block_index, returns, callee_entry, callee_blocks, result_components) = {
            let caller_index = call_splice_index_v1(&caller, budget, &mut scratch)?;
            let callee_index = call_splice_index_v1(&callee, budget, &mut scratch)?;
            if !call_splice_disjoint_v1(
                caller_index.blocks.iter(),
                callee_index.blocks.iter(),
                budget,
            )? || !call_splice_disjoint_v1(
                caller_index.values.iter().map(|row| row.0),
                callee_index.values.iter().map(|row| row.0),
                budget,
            )? {
                return Err(CallInstanceEmissionErrorV1::IdentityRangeOverlap);
            }
            if entry == continuation {
                return Err(CallInstanceEmissionErrorV1::InvalidContinuation);
            }
            for fresh in [entry, continuation] {
                for index in [&caller_index, &callee_index] {
                    budget.charge_work(call_splice_search_work_v1(index.blocks.len()))?;
                    if index.blocks.binary_search(&fresh).is_ok() {
                        return Err(CallInstanceEmissionErrorV1::InvalidContinuation);
                    }
                }
            }
            call_splice_check_body_v1(&caller, &caller_index, false, budget)?;
            let returns = call_splice_check_body_v1(&callee, &callee_index, true, budget)?;
            let caller_body = caller.body.as_ref().unwrap();
            let callee_body = callee.body.as_ref().unwrap();
            budget.charge_work(caller_body.blocks.len())?;
            let block_index = caller_body
                .blocks
                .iter()
                .position(|block| block.id == site.block)
                .ok_or(CallInstanceEmissionErrorV1::MissingCall)?;
            let operation = caller_body.blocks[block_index]
                .operations
                .get(site.operation_index)
                .ok_or(CallInstanceEmissionErrorV1::MissingCall)?;
            let OperationKind::Call {
                callee: target,
                arguments,
            } = &operation.kind
            else {
                return Err(CallInstanceEmissionErrorV1::MissingCall);
            };
            budget.charge_work(
                target
                    .as_str()
                    .len()
                    .checked_add(callee.id.as_str().len())
                    .ok_or_else(call_splice_arithmetic_v1)?,
            )?;
            if target != &callee.id {
                return Err(CallInstanceEmissionErrorV1::WrongCallee);
            }
            if arguments.len() != callee.signature.parameters.len()
                || operation.results.len() != callee.signature.results.len()
            {
                return Err(CallInstanceEmissionErrorV1::SignatureMismatch);
            }
            for (argument, expected) in arguments.iter().zip(&callee.signature.parameters) {
                if !call_splice_type_eq_v1(
                    caller_index.value(*argument, budget)?,
                    expected,
                    budget,
                )? {
                    return Err(CallInstanceEmissionErrorV1::SignatureMismatch);
                }
            }
            for (result, expected) in operation.results.iter().zip(&callee.signature.results) {
                if !call_splice_type_eq_v1(&result.ty, expected, budget)? {
                    return Err(CallInstanceEmissionErrorV1::SignatureMismatch);
                }
            }
            (
                block_index,
                returns,
                callee_body.blocks[0].id,
                callee_body.blocks.len(),
                operation.results.len(),
            )
        };
        // All fallible reservations precede mutation. Existing owned KIR payload
        // moves without cloning; only new vector storage is charged here.
        let caller_body = caller.body.as_mut().unwrap();
        let callee_body = callee.body.as_mut().unwrap();
        let suffix = caller_body.blocks[block_index]
            .operations
            .len()
            .checked_sub(site.operation_index + 1)
            .ok_or_else(call_splice_arithmetic_v1)?;
        let mut tail = call_splice_vec_v1(suffix, budget, &mut retained)?;
        let mut parameters =
            call_splice_vec_v1(callee_body.parameters.len(), budget, &mut retained)?;
        let additional = callee_blocks
            .checked_add(2)
            .ok_or_else(call_splice_arithmetic_v1)?;
        let required = caller_body
            .blocks
            .len()
            .checked_add(additional)
            .ok_or_else(call_splice_arithmetic_v1)?;
        if required > caller_body.blocks.capacity() {
            budget.charge_work(caller_body.blocks.len())?;
            let bytes = required
                .checked_mul(std::mem::size_of::<BasicBlock>())
                .ok_or_else(call_splice_arithmetic_v1)?;
            call_splice_charge_storage_v1(bytes, budget, &mut retained)?;
            caller_body
                .blocks
                .try_reserve_exact(additional)
                .map_err(|_| {
                    CallInstanceEmissionErrorV1::Resource(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Allocation,
                    )
                })?;
        }
        budget.charge_work(
            suffix
                .checked_add(callee_body.parameters.len())
                .and_then(|work| work.checked_add(callee_blocks))
                .and_then(|work| work.checked_add(additional))
                .ok_or_else(call_splice_arithmetic_v1)?,
        )?;
        let block = &mut caller_body.blocks[block_index];
        tail.extend(block.operations.drain(site.operation_index + 1..));
        let call = block.operations.pop().unwrap();
        let OperationKind::Call { arguments, .. } = call.kind else {
            unreachable!()
        };
        let mut continued = BasicBlock::new(continuation);
        continued.parameters = call.results;
        continued.operations = tail;
        continued.terminator = block.terminator.take();
        block.terminator = Some(Terminator::Branch {
            target: entry,
            arguments,
        });
        parameters.extend(
            std::mem::take(&mut callee_body.parameters)
                .into_iter()
                .zip(std::mem::take(&mut callee.signature.parameters))
                .map(|(id, ty)| ValueDef::new(id, ty)),
        );
        let mut preheader = BasicBlock::new(entry);
        preheader.parameters = parameters;
        preheader.terminator = Some(Terminator::Branch {
            target: callee_entry,
            arguments: Vec::new(),
        });
        for block in &mut callee_body.blocks {
            if matches!(block.terminator, Some(Terminator::Return { .. })) {
                let Some(Terminator::Return { values }) = block.terminator.take() else {
                    unreachable!()
                };
                block.terminator = Some(Terminator::Branch {
                    target: continuation,
                    arguments: values,
                });
            }
        }
        caller_body.blocks.push(continued);
        caller_body.blocks.push(preheader);
        caller_body.blocks.append(&mut callee_body.blocks);
        Ok(SplicedCallInstanceV1 {
            caller,
            callee_required_capabilities: callee.required_capabilities,
            split: CallInstanceSplitV1 {
                call: site,
                entry,
                callee_entry,
                continuation,
                callee_blocks,
                returns,
                result_components,
            },
            additional_storage_bytes: retained,
        })
    })();
    budget.release_storage(scratch)?;
    if result.is_err() {
        budget.release_storage(retained)?;
    }
    result
}

#[cfg(test)]
mod call_instance_emission_tests {
    use super::*;

    fn fixture() -> (Function, Function, FunctionOperationLocation, BlockId) {
        let scalar = Type::Scalar(ScalarType::U32);
        let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
        let access = MemoryAccess::new(AddressSpace::Global, 4);
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(1),
                access,
            },
        ));
        block.operations.push(Operation::new(
            vec![ValueDef::new(ValueId(3), scalar.clone())],
            OperationKind::Call {
                callee: FunctionId::new("leaf"),
                arguments: vec![ValueId(1), ValueId(2)],
            },
        ));
        block.operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(3),
                access,
            },
        ));
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(3)],
        });
        let caller = Function::internal_helper(
            "caller",
            Signature::new(
                vec![pointer, scalar.clone(), Type::BOOL],
                vec![scalar.clone()],
            ),
            vec![ValueId(0), ValueId(1), ValueId(2)],
            vec![block],
        );
        let mut entry = BasicBlock::new(BlockId(17));
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(101),
            then_target: BlockId(18),
            then_arguments: vec![],
            else_target: BlockId(19),
            else_arguments: vec![],
        });
        let mut left = BasicBlock::new(BlockId(18));
        left.terminator = Some(Terminator::Return {
            values: vec![ValueId(100)],
        });
        let mut right = BasicBlock::new(BlockId(19));
        right.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(102), scalar.clone()),
            OperationKind::Binary {
                op: BinaryOp::BitOr,
                lhs: ValueId(100),
                rhs: ValueId(100),
            },
        ));
        right.terminator = Some(Terminator::Return {
            values: vec![ValueId(102)],
        });
        let callee = Function::internal_helper(
            "leaf",
            Signature::new(vec![scalar.clone(), Type::BOOL], vec![scalar]),
            vec![ValueId(100), ValueId(101)],
            vec![entry, left, right],
        );
        (
            caller,
            callee,
            FunctionOperationLocation::new(BlockId(0), 1),
            BlockId(20),
        )
    }

    fn run(
        caller: Function,
        callee: Function,
        site: FunctionOperationLocation,
        continuation: BlockId,
    ) -> Result<SplicedCallInstanceV1, CallInstanceEmissionErrorV1> {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
        splice_production_call_instance_v1(
            caller,
            callee,
            site,
            BlockId(16),
            continuation,
            &mut budget,
        )
    }

    #[test]
    fn splice_preserves_effect_order_and_redirects_each_return() {
        let (caller, callee, site, continuation) = fixture();
        let result = run(caller, callee, site, continuation).unwrap();
        assert_eq!(result.split.returns, 2);
        assert_eq!(result.split.result_components, 1);
        assert!(result.callee_required_capabilities.is_empty());
        let blocks = &result.caller.body.as_ref().unwrap().blocks;
        assert_eq!(blocks[0].operations.len(), 1);
        assert!(matches!(
            blocks[0].operations[0].kind,
            OperationKind::Store {
                value: ValueId(1),
                ..
            }
        ));
        assert!(matches!(&blocks[0].terminator,
            Some(Terminator::Branch { target: BlockId(16), arguments })
                if arguments == &[ValueId(1), ValueId(2)]));
        assert_eq!(blocks[1].id, continuation);
        assert_eq!(
            blocks[1].parameters,
            [ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32))]
        );
        assert!(matches!(
            blocks[1].operations[0].kind,
            OperationKind::Store {
                value: ValueId(3),
                ..
            }
        ));
        assert!(matches!(&blocks[1].terminator,
            Some(Terminator::Return { values }) if values == &[ValueId(3)]));
        assert_eq!(
            blocks[2]
                .parameters
                .iter()
                .map(|value| value.id)
                .collect::<Vec<_>>(),
            [ValueId(100), ValueId(101)]
        );
        assert!(blocks[3].parameters.is_empty());
        assert!(matches!(&blocks[2].terminator,
            Some(Terminator::Branch { target: BlockId(17), arguments }) if arguments.is_empty()));
        for block in &blocks[4..] {
            assert!(matches!(&block.terminator,
                Some(Terminator::Branch { target: BlockId(20), arguments }) if arguments.len() == 1));
        }
        let mut module = Module::new("spliced");
        module.functions.push(result.caller);
        verify_module(&module).unwrap();
    }

    #[test]
    fn splice_accepts_zero_results_and_flattened_multiple_results() {
        for count in [0, 2] {
            let (mut caller, mut callee, site, continuation) = fixture();
            let scalar = Type::Scalar(ScalarType::U32);
            callee.signature.results = vec![scalar.clone(); count];
            let caller_body = caller.body.as_mut().unwrap();
            caller_body.blocks[0].operations[1].results = (0..count)
                .map(|slot| ValueDef::new(ValueId(3 + slot as u32), scalar.clone()))
                .collect();
            if count == 0 {
                caller.signature.results.clear();
                caller_body.blocks[0].operations.pop();
                caller_body.blocks[0].terminator = Some(Terminator::Return { values: vec![] });
            }
            for block in &mut callee.body.as_mut().unwrap().blocks {
                if let Some(Terminator::Return { values }) = &mut block.terminator {
                    values.resize(count, ValueId(100));
                }
            }
            let result = run(caller, callee, site, continuation).unwrap();
            assert_eq!(result.split.result_components, count);
            let mut module = Module::new("results");
            module.functions.push(result.caller);
            verify_module(&module).unwrap();
        }
    }

    #[test]
    fn splice_rejects_wrong_bindings_and_nonfresh_identities() {
        let (caller, mut callee, site, continuation) = fixture();
        callee.id = FunctionId::new("other");
        assert!(matches!(
            run(caller, callee, site, continuation),
            Err(CallInstanceEmissionErrorV1::WrongCallee)
        ));
        let (caller, mut callee, site, continuation) = fixture();
        callee.signature.parameters[1] = Type::Scalar(ScalarType::U64);
        assert!(matches!(
            run(caller, callee, site, continuation),
            Err(CallInstanceEmissionErrorV1::SignatureMismatch)
        ));
        let (caller, callee, site, _) = fixture();
        assert!(matches!(
            run(caller, callee, site, BlockId(18)),
            Err(CallInstanceEmissionErrorV1::InvalidContinuation)
        ));
        let (caller, mut callee, site, continuation) = fixture();
        callee.body.as_mut().unwrap().parameters[0] = ValueId(1);
        assert!(matches!(
            run(caller, callee, site, continuation),
            Err(CallInstanceEmissionErrorV1::IdentityRangeOverlap)
        ));
        let (caller, callee, _, continuation) = fixture();
        assert!(matches!(
            run(
                caller,
                callee,
                FunctionOperationLocation::new(BlockId(0), 0),
                continuation
            ),
            Err(CallInstanceEmissionErrorV1::MissingCall)
        ));
    }

    #[test]
    fn splice_accepts_disjoint_nonordered_block_and_value_ranges() {
        let (mut caller, callee, _, continuation) = fixture();
        caller
            .signature
            .parameters
            .push(Type::Scalar(ScalarType::U32));
        let body = caller.body.as_mut().unwrap();
        body.parameters.push(ValueId(1000));
        body.blocks[0].id = BlockId(30);
        let result = run(
            caller,
            callee,
            FunctionOperationLocation::new(BlockId(30), 1),
            continuation,
        )
        .unwrap();
        assert_eq!(result.split.entry, BlockId(16));
        assert_eq!(result.split.callee_entry, BlockId(17));
        let mut module = Module::new("disjoint");
        module.functions.push(result.caller);
        verify_module(&module).unwrap();
    }

    #[test]
    fn splice_rejects_foreign_values_and_preserves_entry_backedges() {
        let (caller, mut callee, site, continuation) = fixture();
        callee.body.as_mut().unwrap().blocks[1].terminator = Some(Terminator::Return {
            values: vec![ValueId(1)],
        });
        assert!(matches!(
            run(caller, callee, site, continuation),
            Err(CallInstanceEmissionErrorV1::MissingDefinition)
        ));
        let (caller, mut callee, site, continuation) = fixture();
        callee.body.as_mut().unwrap().blocks[1].terminator = Some(Terminator::Branch {
            target: BlockId(17),
            arguments: vec![],
        });
        let result = run(caller, callee, site, continuation).unwrap();
        let mut module = Module::new("backedge");
        module.functions.push(result.caller);
        verify_module(&module).unwrap();
    }

    #[test]
    fn splice_rejects_execution_results_even_without_a_return() {
        let (mut caller, callee, site, continuation) = fixture();
        caller.signature.results =
            vec![Type::Execution(fe2o3_kernel_ir::ExecutionRoleV15::Context)];
        caller.body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Unreachable);
        assert!(matches!(
            run(caller, callee, site, continuation),
            Err(CallInstanceEmissionErrorV1::ExecutionTransport)
        ));
        let (caller, mut callee, site, continuation) = fixture();
        callee.body.as_mut().unwrap().blocks[1].terminator =
            Some(Terminator::Return { values: vec![] });
        assert!(matches!(
            run(caller, callee, site, continuation),
            Err(CallInstanceEmissionErrorV1::SignatureMismatch)
        ));
    }

    #[test]
    fn splice_rejects_callee_frame_allocations_including_repeated_calls() {
        for repeated in [false, true] {
            let (mut caller, mut callee, site, continuation) = fixture();
            let scalar = Type::Scalar(ScalarType::U32);
            let access = MemoryAccess::new(AddressSpace::Private, 4);
            let body = callee.body.as_mut().unwrap();
            body.blocks[0].operations = vec![
                Operation::effect_free(
                    ValueDef::new(
                        ValueId(103),
                        Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite),
                    ),
                    OperationKind::Alloca {
                        element: scalar.clone(),
                        count: None,
                        address_space: AddressSpace::Private,
                        alignment: 4,
                    },
                ),
                Operation::new(
                    vec![],
                    OperationKind::Store {
                        pointer: ValueId(103),
                        value: ValueId(100),
                        access,
                    },
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(104), scalar),
                    OperationKind::Load {
                        pointer: ValueId(103),
                        access,
                    },
                ),
            ];
            body.blocks[1].terminator = Some(Terminator::Return {
                values: vec![ValueId(104)],
            });
            if repeated {
                let body = caller.body.as_mut().unwrap();
                let mut returned = BasicBlock::new(BlockId(1));
                returned.terminator = body.blocks[0].terminator.take();
                body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
                    condition: ValueId(2),
                    then_target: BlockId(0),
                    then_arguments: vec![],
                    else_target: BlockId(1),
                    else_arguments: vec![],
                });
                body.blocks.push(returned);
            }
            let mut source = Module::new("frame_allocation");
            source.functions = vec![caller.clone(), callee.clone()];
            verify_module(&source).unwrap();
            assert!(matches!(
                run(caller, callee, site, continuation),
                Err(CallInstanceEmissionErrorV1::CalleeFrameAllocation)
            ));
        }
    }

    #[test]
    fn splice_preserves_caller_frame_allocations() {
        let (mut caller, callee, site, continuation) = fixture();
        let scalar = Type::Scalar(ScalarType::U32);
        caller.body.as_mut().unwrap().blocks[0]
            .operations
            .push(Operation::effect_free(
                ValueDef::new(
                    ValueId(4),
                    Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite),
                ),
                OperationKind::Alloca {
                    element: scalar,
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            ));
        let result = run(caller, callee, site, continuation).unwrap();
        let mut module = Module::new("caller_frame");
        module.functions.push(result.caller);
        verify_module(&module).unwrap();
    }

    #[test]
    fn splice_preserves_workgroup_allocation_declaration_origins() {
        for in_callee in [true, false] {
            let (mut caller, mut callee, site, continuation) = fixture();
            let scalar = Type::Scalar(ScalarType::U32);
            let allocation = Operation::effect_free(
                ValueDef::new(
                    ValueId(if in_callee { 103 } else { 4 }),
                    Type::pointer(
                        scalar.clone(),
                        AddressSpace::Workgroup,
                        AccessMode::ReadWrite,
                    ),
                ),
                OperationKind::WorkgroupMemory(fe2o3_kernel_ir::WorkgroupMemory {
                    element: scalar,
                    extent: fe2o3_kernel_ir::WorkgroupMemoryExtent::Static(1),
                    alignment: 4,
                }),
            );
            let function = if in_callee { &mut callee } else { &mut caller };
            function.body.as_mut().unwrap().blocks[0]
                .operations
                .push(allocation);
            let mut source = Module::new("workgroup_allocation");
            source.functions = vec![caller.clone(), callee.clone()];
            verify_module(&source).unwrap();
            let result = run(caller, callee, site, continuation);
            if in_callee {
                assert!(matches!(
                    result,
                    Err(CallInstanceEmissionErrorV1::CalleeWorkgroupAllocation)
                ));
            } else {
                let mut module = Module::new("caller_workgroup_allocation");
                module.functions.push(result.unwrap().caller);
                verify_module(&module).unwrap();
            }
        }
    }

    #[test]
    fn splice_requires_collective_occurrence_proof_but_preserves_local_effects() {
        use fe2o3_kernel_ir::{
            Barrier, BarrierSemantics, Convergence, Fence, IntrinsicOperation, MemoryOrdering,
            SynchronizationScope, WaveOperation, WaveOperationKind, WaveWidth, WorkgroupBarrier,
        };
        let scope = SynchronizationScope::Workgroup;
        let semantics =
            BarrierSemantics::new(MemoryOrdering::AcquireRelease, [AddressSpace::Workgroup]);
        for (kind, ty, collective) in [
            (
                OperationKind::Barrier(Barrier {
                    execution_scope: scope,
                    memory_scope: scope,
                    semantics: semantics.clone(),
                }),
                None,
                true,
            ),
            (
                OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                    memory_scope: scope,
                    semantics: semantics.clone(),
                    convergence: Convergence::uniform(scope),
                }),
                None,
                true,
            ),
            (
                OperationKind::Wave(WaveOperation::full(
                    WaveOperationKind::LaneId,
                    WaveWidth::Wave64,
                )),
                Some(Type::Scalar(ScalarType::U32)),
                true,
            ),
            (
                OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
                Some(Type::INDEX),
                false,
            ),
            (
                OperationKind::Fence(Fence {
                    memory_scope: scope,
                    semantics,
                }),
                None,
                false,
            ),
        ] {
            let (caller, mut callee, site, continuation) = fixture();
            let results = ty
                .map(|ty| vec![ValueDef::new(ValueId(103), ty)])
                .unwrap_or_default();
            callee.body.as_mut().unwrap().blocks[0]
                .operations
                .push(Operation::new(results, kind));
            let mut source = Module::new("call_effects");
            source.functions = vec![caller.clone(), callee.clone()];
            verify_module(&source).unwrap();
            let result = run(caller, callee, site, continuation);
            if collective {
                assert!(matches!(
                    result,
                    Err(CallInstanceEmissionErrorV1::CalleeCollective)
                ));
            } else {
                let mut module = Module::new("local_effects");
                module.functions.push(result.unwrap().caller);
                verify_module(&module).unwrap();
            }
        }
    }

    #[test]
    fn splice_preserves_nested_retained_calls() {
        let (caller, mut callee, site, continuation) = fixture();
        let mut nested = callee.clone();
        nested.id = FunctionId::new("nested");
        callee.body.as_mut().unwrap().blocks[0]
            .operations
            .push(Operation::effect_free(
                ValueDef::new(ValueId(103), Type::Scalar(ScalarType::U32)),
                OperationKind::Call {
                    callee: nested.id.clone(),
                    arguments: vec![ValueId(100), ValueId(101)],
                },
            ));
        let mut source = Module::new("nested_source");
        source.functions = vec![caller.clone(), callee.clone(), nested.clone()];
        verify_module(&source).unwrap();
        let result = run(caller, callee, site, continuation).unwrap();
        let mut module = Module::new("nested_result");
        module.functions = vec![result.caller, nested];
        verify_module(&module).unwrap();
    }

    #[test]
    fn splice_budget_is_cumulative_and_preserves_the_owner_floor() {
        let measure = |work_limit, storage_limit| {
            let (caller, callee, site, continuation) = fixture();
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(7).unwrap();
            let result = splice_production_call_instance_v1(
                caller,
                callee,
                site,
                BlockId(16),
                continuation,
                &mut budget,
            );
            (
                result,
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
            )
        };
        let (result, exact_work, live, peak) = measure(1_000_000, 1_000_000);
        let result = result.unwrap();
        assert_eq!(live, 7 + result.additional_storage_bytes);
        assert!(peak >= live);
        assert!(measure(exact_work, peak).0.is_ok());
        let (failed, _, remaining, _) = measure(exact_work - 1, peak);
        assert!(matches!(
            failed,
            Err(CallInstanceEmissionErrorV1::Resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(_)
            ))
        ));
        assert_eq!(remaining, 7);
        let (failed, _, remaining, _) = measure(exact_work, peak - 1);
        assert!(matches!(
            failed,
            Err(CallInstanceEmissionErrorV1::Resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Storage(_)
            ))
        ));
        assert_eq!(remaining, 7);
    }
}
