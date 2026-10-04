//! Bounded actual SSA dependency projection. Values are analysis-only, never
//! scalar execution, physical register state or a replacement canonical graph.
use super::*;
use fe2o3_kernel_ir::{
    BinaryOp, BlockId, CastKind, Constant, FunctionBody, Gfx942ProgramInstructionV1,
    Gfx942ProgramRoleV1, OperationKind as Op, ScalarType, Terminator as End, Type, UnaryOp,
};

#[path = "production_ordered_composition_control_v1.rs"]
mod control;

#[derive(Clone, Copy, Eq, PartialEq)]
struct Key {
    function: u32,
    call: Option<OrderedProgramCallKeyV1>,
    value: ValueId,
}
#[derive(Clone, Copy)]
struct Memo {
    key: Key,
    value: Option<ValueR>,
}
pub(super) fn memo_storage_bound() -> Result<usize, E> {
    Ok(argument_product_v1(MAX_NODES, std::mem::size_of::<Memo>())?)
}
// Bound the logical operand/role/key payload of the bounded recursive walk.
// This is not compiler call-frame size or a whole-process stack/RSS claim.
pub(super) fn scratch_storage_bound() -> Result<usize, E> {
    let per_depth = argument_sum_v1(&[
        argument_product_v1(3 * MAX_DEPENDENCIES, std::mem::size_of::<ValueR>())?,
        argument_product_v1(2 * MAX_DEPENDENCIES, std::mem::size_of::<ValueId>())?,
        256,
    ])?;
    Ok(argument_product_v1(256, per_depth)?)
}
pub(super) struct Dependencies<'o, 'b, 'w> {
    owner: &'o VerifiedOrderedProgramCompositionV1,
    budget: &'b mut ArgumentBudgetV1<'w>,
    operations: Vec<OpR>,
    rows: Vec<OrderedCompositionRankedDependencyV1>,
    memo: Vec<Memo>,
    next: u32,
}
impl<'o, 'b, 'w> Dependencies<'o, 'b, 'w> {
    pub(super) fn new(
        owner: &'o VerifiedOrderedProgramCompositionV1,
        budget: &'b mut ArgumentBudgetV1<'w>,
    ) -> Result<Self, E> {
        Ok(Self {
            owner,
            budget,
            operations: vector(MAX_NODES)?,
            rows: vector(MAX_NODES)?,
            memo: vector(MAX_NODES)?,
            next: 0,
        })
    }
    pub(super) fn next(&mut self) -> Result<IdR, E> {
        if self.next as usize >= MAX_NODES {
            return Err(refusal("ranked value bound"));
        }
        let id = IdR::new(self.next);
        self.next += 1;
        Ok(id)
    }
    pub(super) fn push(&mut self, operation: OpR) -> Result<(), E> {
        self.budget.charge_work(1)?;
        if self.operations.len() == MAX_NODES {
            return Err(refusal("ranked operation bound"));
        }
        self.operations.push(operation);
        Ok(())
    }
    fn join(&mut self, values: &[ValueR]) -> Result<ValueR, E> {
        if values.len() > MAX_DEPENDENCIES {
            return Err(refusal("ranked dependency arity"));
        }
        let result = self.next()?;
        self.push(if values.is_empty() {
            OpR::IndexUnknown { result }
        } else {
            OpR::DeterministicJoin {
                result,
                dependencies: copy(values)?,
            }
        })?;
        Ok(ValueR::Local(result))
    }
    fn row(
        &mut self,
        key: Key,
        block: BlockId,
        operation: Option<u32>,
        descriptor: Option<u8>,
        value: ValueR,
    ) -> Result<(), E> {
        let ValueR::Local(ranked) = value else {
            return Err(refusal("local dependency attribution"));
        };
        if self.rows.len() == MAX_NODES {
            return Err(refusal("ranked attribution bound"));
        }
        self.rows.push(OrderedCompositionRankedDependencyV1 {
            function: key.function,
            call: key.call,
            block,
            operation,
            result: key.value,
            descriptor,
            ranked,
        });
        Ok(())
    }
    pub(super) fn finish(self) -> (Vec<OpR>, Vec<OrderedCompositionRankedDependencyV1>) {
        (self.operations, self.rows)
    }
    fn scalar(ty: &Type) -> bool {
        matches!(
            ty,
            Type::Scalar(
                ScalarType::Bool
                    | ScalarType::U8
                    | ScalarType::U16
                    | ScalarType::U32
                    | ScalarType::U64
                    | ScalarType::Index
            )
        )
    }
    fn body(&self, function: u32) -> Result<&'o FunctionBody, E> {
        self.owner
            .canonical()
            .module()
            .functions
            .get(function as usize)
            .and_then(|f| f.body.as_ref())
            .ok_or_else(|| refusal("actual dependency function"))
    }
    fn scalar_value(&mut self, function: u32, value: ValueId) -> Result<bool, E> {
        let module = self.owner.canonical().module();
        let f = module
            .functions
            .get(function as usize)
            .ok_or_else(|| refusal("function coordinate"))?;
        let b = f.body.as_ref().ok_or_else(|| refusal("body"))?;
        self.budget.charge_work(b.parameters.len())?;
        if let Some(i) = b.parameters.iter().position(|v| *v == value) {
            return Ok(Self::scalar(&f.signature.parameters[i]));
        }
        for block in &b.blocks {
            self.budget
                .charge_work(block.parameters.len() + block.operations.len())?;
            if let Some(v) = block.parameters.iter().find(|v| v.id == value) {
                return Ok(Self::scalar(&v.ty));
            }
            for op in &block.operations {
                self.budget.charge_work(op.results.len())?;
                if let Some(v) = op.results.iter().find(|v| v.id == value) {
                    return Ok(Self::scalar(&v.ty) || self.boolean_discriminant(function, op)?);
                }
            }
        }
        Err(refusal("unknown actual SSA dependency"))
    }
    pub(super) fn root_dependencies(&mut self) -> Result<(), E> {
        let function = self.owner.root_function_ordinal();
        let body = self.body(function)?;
        self.root_control(body)?;
        for block in &body.blocks {
            for operation in &block.operations {
                // Pointer transport remains in formal derivation. Scalar totality
                // is not inferred from a generic unknown operation.
                self.validate(operation)?;
                for result in &operation.results {
                    if Self::scalar(&result.ty) || self.boolean_discriminant(function, operation)? {
                        self.value(
                            Key {
                                function,
                                call: None,
                                value: result.id,
                            },
                            0,
                        )?;
                    } else if !matches!(
                        operation.kind,
                        Op::SliceData { .. }
                            | Op::GetElementPointer { .. }
                            | Op::Select { .. }
                            | Op::Cast {
                                kind: CastKind::RestrictPointerAccess,
                                ..
                            }
                    ) {
                        return Err(refusal("non-scalar root result projection"));
                    }
                }
            }
        }
        // Dead/repeated authored definitions still execute and retain attribution.
        let owner = self.owner;
        for occurrence in owner.occurrences() {
            let definition = owner
                .definitions()
                .iter()
                .find(|d| d.key() == occurrence.definition())
                .ok_or_else(|| refusal("actual region definition"))?;
            let site = definition.site();
            let op = &self.body(site.function_ordinal())?.blocks[site.block_ordinal() as usize]
                .operations[site.operation_ordinal() as usize];
            let [result] = op.results.as_slice() else {
                return Err(refusal("region result arity"));
            };
            self.value(
                Key {
                    function: site.function_ordinal(),
                    call: occurrence.incoming_call(),
                    value: result.id,
                },
                0,
            )?;
        }
        if self.rows.iter().filter(|r| r.descriptor.is_some()).count()
            != owner.expanded_instruction_count()
        {
            return Err(refusal("complete authored dependency census"));
        }
        Ok(())
    }
    fn root_control(&mut self, body: &FunctionBody) -> Result<(), E> {
        if body.blocks.is_empty() || body.blocks.len() > 128 {
            return Err(refusal("root CFG bound"));
        }
        let mut incoming = [0usize; 128];
        for block in &body.blocks {
            let targets = self.root_successors(block)?;
            for target in targets.into_iter().flatten() {
                self.budget.charge_work(body.blocks.len())?;
                let i = body
                    .blocks
                    .iter()
                    .position(|b| b.id == target)
                    .ok_or_else(|| refusal("root CFG target"))?;
                incoming[i] = incoming[i]
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
            }
        }
        let mut processed = [false; 128];
        for _ in 0..body.blocks.len() {
            self.budget.charge_work(body.blocks.len())?;
            let i = (0..body.blocks.len())
                .find(|i| !processed[*i] && incoming[*i] == 0)
                .ok_or_else(|| refusal("root loop needs unsupported ranked control proof"))?;
            processed[i] = true;
            let targets = self.root_successors(&body.blocks[i])?;
            for target in targets.into_iter().flatten() {
                self.budget.charge_work(body.blocks.len())?;
                let j = body
                    .blocks
                    .iter()
                    .position(|b| b.id == target)
                    .ok_or_else(|| refusal("root target"))?;
                incoming[j] = incoming[j]
                    .checked_sub(1)
                    .ok_or(ArgumentResourceV1::Accounting)?;
            }
        }
        Ok(())
    }
    fn validate(&self, operation: &fe2o3_kernel_ir::Operation) -> Result<(), E> {
        match &operation.kind {
            Op::Constant(c) if Self::scalar(&c.ty()) => Ok(()),
            Op::Intrinsic(_)
            | Op::Unary {
                op: UnaryOp::Not, ..
            }
            | Op::Binary {
                op: BinaryOp::Checked(_) | BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor,
                ..
            }
            | Op::Compare { .. }
            | Op::Select { .. }
            | Op::Call { .. }
            | Op::Gfx942OrderedProgram(_)
            | Op::Cast {
                kind:
                    CastKind::Truncate
                    | CastKind::ZeroExtend
                    | CastKind::Bitcast
                    | CastKind::RestrictPointerAccess,
                ..
            }
            | Op::SliceLength { .. }
            | Op::SliceData { .. }
            | Op::GetElementPointer { .. }
            | Op::Load { .. }
            | Op::Store { .. }
            | Op::GuardedLoad { .. }
            | Op::GuardedStore { .. } => Ok(()),
            _ => Err(refusal(
                "unsupported or partial scalar operation in safety projection",
            )),
        }
    }
    fn value(&mut self, key: Key, depth: usize) -> Result<ValueR, E> {
        self.budget.charge_work(self.memo.len() + 1)?;
        if depth >= 256 {
            return Err(refusal("ranked dependency depth"));
        }
        if let Some(row) = self.memo.iter().find(|row| row.key == key) {
            return row
                .value
                .ok_or_else(|| refusal("cyclic dependency requires unsupported loop analysis"));
        }
        if self.memo.len() == MAX_NODES {
            return Err(refusal("ranked memo bound"));
        }
        let position = self.memo.len();
        self.memo.push(Memo { key, value: None });
        let body = self.body(key.function)?;
        let result = if let Some(parameter) = body.parameters.iter().position(|v| *v == key.value) {
            if let Some(call) = key.call {
                let row = self
                    .owner
                    .calls()
                    .iter()
                    .find(|c| c.key() == call)
                    .ok_or_else(|| refusal("actual helper invocation"))?;
                let site = row.site();
                let op = &self.body(site.function_ordinal())?.blocks[site.block_ordinal() as usize]
                    .operations[site.operation_ordinal() as usize];
                let Op::Call { arguments, .. } = &op.kind else {
                    return Err(refusal("actual call opcode"));
                };
                let argument = *arguments
                    .get(parameter)
                    .ok_or_else(|| refusal("actual helper argument"))?;
                self.value(
                    Key {
                        function: site.function_ordinal(),
                        call: None,
                        value: argument,
                    },
                    depth + 1,
                )?
            } else {
                if key.function != self.owner.root_function_ordinal() {
                    return Err(refusal("helper value without actual call"));
                }
                ValueR::Argument(parameter as u32)
            }
        } else {
            let mut found = None;
            for block in &body.blocks {
                self.budget
                    .charge_work(block.operations.len() + block.parameters.len() + 1)?;
                if let Some(index) = block.parameters.iter().position(|p| p.id == key.value) {
                    let value = self.phi(key, block.id, index, depth + 1)?;
                    found = Some(value);
                    break;
                }
                for (oi, operation) in block.operations.iter().enumerate() {
                    if operation.results.iter().any(|v| v.id == key.value) {
                        self.validate(operation)?;
                        let value =
                            self.operation(key, block.id, oi as u32, operation, depth + 1)?;
                        found = Some(value);
                        break;
                    }
                }
                if found.is_some() {
                    break;
                }
            }
            found.ok_or_else(|| refusal("actual dependency definition absent"))?
        };
        self.memo[position].value = Some(result);
        Ok(result)
    }
    fn phi(&mut self, key: Key, block: BlockId, index: usize, depth: usize) -> Result<ValueR, E> {
        let body = self.body(key.function)?;
        let mut ids = [ValueId(0); MAX_DEPENDENCIES];
        let mut count = 0usize;
        for source in &body.blocks {
            self.budget.charge_work(1)?;
            let mut edge =
                |target: BlockId, args: &[ValueId], condition: Option<ValueId>| -> Result<(), E> {
                    if target != block {
                        return Ok(());
                    }
                    let value = *args
                        .get(index)
                        .ok_or_else(|| refusal("actual phi edge argument"))?;
                    for v in [Some(value), condition].into_iter().flatten() {
                        if count == ids.len() {
                            return Err(refusal("phi dependency bound"));
                        }
                        ids[count] = v;
                        count += 1;
                    }
                    Ok(())
                };
            match source
                .terminator
                .as_ref()
                .ok_or_else(|| refusal("actual terminator"))?
            {
                End::Branch { target, arguments } => edge(*target, arguments, None)?,
                End::ConditionalBranch {
                    condition,
                    then_target,
                    then_arguments,
                    else_target,
                    else_arguments,
                } => {
                    edge(*then_target, then_arguments, Some(*condition))?;
                    edge(*else_target, else_arguments, Some(*condition))?;
                }
                End::Switch {
                    selector,
                    cases,
                    default_target,
                    default_arguments,
                } => {
                    // root_control already checked the exact two-case Bool
                    // discriminant shape. Retain every actual predecessor, even
                    // when this switch is unrelated to the selected phi.
                    for case in cases {
                        edge(case.target, &case.arguments, Some(*selector))?;
                    }
                    edge(*default_target, default_arguments, Some(*selector))?;
                }
                End::Return { .. } | End::Unreachable => {}
                _ => return Err(refusal("unsupported phi predecessor control")),
            }
        }
        if count == 0 {
            return Err(refusal("phi without incoming edges"));
        }
        let mut values = [ValueR::Argument(0); MAX_DEPENDENCIES];
        for (i, id) in ids[..count].iter().enumerate() {
            values[i] = self.value(Key { value: *id, ..key }, depth + 1)?;
        }
        let value = self.join(&values[..count])?;
        self.row(key, block, None, None, value)?;
        Ok(value)
    }
    fn operation(
        &mut self,
        key: Key,
        block: BlockId,
        oi: u32,
        operation: &fe2o3_kernel_ir::Operation,
        depth: usize,
    ) -> Result<ValueR, E> {
        let value = match &operation.kind {
            Op::Gfx942OrderedProgram(program) => {
                let mut roles = [None; 5];
                for (i, input) in program.inputs().iter().enumerate() {
                    roles[i] = Some(self.value(
                        Key {
                            value: *input,
                            ..key
                        },
                        depth + 1,
                    )?);
                }
                for (ordinal, instruction) in program.program().instructions().enumerate() {
                    let get = |role: Gfx942ProgramRoleV1| {
                        roles[role as usize].ok_or_else(|| refusal("authored role initialization"))
                    };
                    let (values, count) = match instruction {
                        Gfx942ProgramInstructionV1::Move { source, .. } => {
                            ([get(source)?, ValueR::Argument(0)], 1)
                        }
                        Gfx942ProgramInstructionV1::Binary { left, right, .. } => {
                            ([get(left)?, get(right)?], 2)
                        }
                    };
                    let result = self.join(&values[..count])?;
                    self.row(key, block, Some(oi), Some(ordinal as u8), result)?;
                    roles[instruction.destination().role() as usize] = Some(result);
                }
                roles[Gfx942ProgramRoleV1::Output as usize]
                    .ok_or_else(|| refusal("authored output"))?
            }
            Op::Call { callee, .. } => {
                if key.call.is_some() {
                    return Err(refusal("nested composition call"));
                }
                let row = self
                    .owner
                    .calls()
                    .iter()
                    .find(|row| {
                        let site = row.site();
                        site.function_ordinal() == key.function
                            && site.block() == block
                            && site.operation_ordinal() == oi
                    })
                    .ok_or_else(|| refusal("exact root call coordinate"))?;
                let helper = self
                    .owner
                    .helpers()
                    .iter()
                    .find(|h| h.key() == row.callee())
                    .ok_or_else(|| refusal("actual structural helper"))?;
                let function =
                    &self.owner.canonical().module().functions[helper.function_ordinal() as usize];
                if &function.id != callee {
                    return Err(refusal("helper callee identity"));
                }
                let mut returned = None;
                for block in &self.body(helper.function_ordinal())?.blocks {
                    if let Some(End::Return { values }) = &block.terminator {
                        let [value] = values.as_slice() else {
                            return Err(refusal("helper return arity"));
                        };
                        if returned.replace(*value).is_some() {
                            return Err(refusal("multiple helper returns"));
                        }
                    }
                }
                self.value(
                    Key {
                        function: helper.function_ordinal(),
                        call: Some(row.key()),
                        value: returned.ok_or_else(|| refusal("helper return absent"))?,
                    },
                    depth + 1,
                )?
            }
            Op::Constant(constant) => {
                let number = match constant {
                    Constant::Bool(v) => u64::from(*v),
                    Constant::U8(v) => u64::from(*v),
                    Constant::U16(v) => u64::from(*v),
                    Constant::U32(v) => u64::from(*v),
                    Constant::U64(v) | Constant::Index(v) => *v,
                    _ => return Err(refusal("unsupported scalar constant")),
                };
                let result = self.next()?;
                self.push(OpR::IndexConstant {
                    result,
                    value: number,
                })?;
                ValueR::Local(result)
            }
            Op::Load { .. }
            | Op::GuardedLoad { .. }
            | Op::SliceLength { .. }
            | Op::Intrinsic(_) => {
                let result = self.next()?;
                self.push(OpR::IndexUnknown { result })?;
                ValueR::Local(result)
            }
            _ => {
                let mut ids = [ValueId(0); MAX_DEPENDENCIES];
                let mut count = 0usize;
                operation.kind.try_visit_operands(|id| {
                    if count == ids.len() {
                        return Err(refusal("actual operand bound"));
                    }
                    ids[count] = id;
                    count += 1;
                    Ok(())
                })?;
                let mut values = [ValueR::Argument(0); MAX_DEPENDENCIES];
                let mut length = 0;
                for id in &ids[..count] {
                    if self.scalar_value(key.function, *id)? {
                        values[length] = self.value(Key { value: *id, ..key }, depth + 1)?;
                        length += 1;
                    }
                }
                self.join(&values[..length])?
            }
        };
        // The call/program result may alias the existing projected return. The
        // attribution still records its actual original result and static site.
        let value = match value {
            ValueR::Local(_) => value,
            _ => self.join(&[value])?,
        };
        self.row(key, block, Some(oi), None, value)?;
        Ok(value)
    }
    pub(super) fn check_access(
        &mut self,
        access: &fe2o3_kernel_ir::FormalMemoryAccess,
    ) -> Result<(), E> {
        let body = self.body(self.owner.root_function_ordinal())?;
        let location = access.location();
        let op = body
            .blocks
            .iter()
            .find(|b| b.id == location.block)
            .and_then(|b| b.operations.get(location.operation_index))
            .ok_or_else(|| refusal("actual formal access location"))?;
        let expected = match op.kind {
            Op::Store { .. } | Op::GuardedStore { .. } => FormalMemoryAccessKind::Write,
            Op::Load { .. } | Op::GuardedLoad { .. } => FormalMemoryAccessKind::Read,
            _ => return Err(refusal("actual formal memory opcode")),
        };
        if expected != access.kind() {
            return Err(refusal("actual memory effect differs"));
        }
        if let Op::Store { value, .. } | Op::GuardedStore { value, .. } = &op.kind {
            self.value(
                Key {
                    function: self.owner.root_function_ordinal(),
                    call: None,
                    value: *value,
                },
                0,
            )?;
        }
        self.budget.charge_work(1)?;
        Ok(())
    }
}
