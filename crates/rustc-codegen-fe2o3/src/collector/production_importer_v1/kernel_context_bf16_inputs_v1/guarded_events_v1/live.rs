//! Real guarded read SSA for one original terminal's four source subevents.
//! This source-event projection is not a final KIR or allocation proof.
use super::formula::{
    self, Binary, Charge, Compare, Error, Formula, INPUTS, Id, NODES, Node, Result,
};
use dialect_kernel::*;
use pliron::{
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp, types::FunctionType},
    context::{Context, IrMutationAttemptEpoch, Ptr},
    dialect::DialectName,
    linked_list::ContainsLinkedList,
    op::Op,
    operation::Operation,
    value::Value,
};

#[path = "read_observations.rs"]
pub(super) mod read_observations;

const OPERATIONS: usize = 2 * NODES + 2 * INPUTS + 16;
fn scalar(bits: u16) -> SemanticTypedScalarV1 {
    SemanticTypedScalarV1::new(SemanticScalarKindAttr::UnsignedInteger, bits).unwrap()
}
fn boolean() -> SemanticTypedScalarV1 {
    SemanticTypedScalarV1::new(SemanticScalarKindAttr::Bool, 1).unwrap()
}
fn binary(op: Binary) -> SemanticTypedBinaryKindAttr {
    match op {
        Binary::Add => SemanticTypedBinaryKindAttr::Add,
        Binary::Subtract => SemanticTypedBinaryKindAttr::Subtract,
        Binary::Multiply => SemanticTypedBinaryKindAttr::Multiply,
        Binary::Divide => SemanticTypedBinaryKindAttr::Divide,
        Binary::Remainder => SemanticTypedBinaryKindAttr::Remainder,
    }
}
fn compare(op: Compare) -> SemanticTypedCompareKindAttr {
    match op {
        Compare::Less => SemanticTypedCompareKindAttr::LessThan,
        Compare::LessEqual => SemanticTypedCompareKindAttr::LessOrEqual,
        Compare::Equal => SemanticTypedCompareKindAttr::Equal,
    }
}
fn index_binary(op: Binary) -> Result<IndexBinaryKindAttr> {
    Ok(match op {
        Binary::Add => IndexBinaryKindAttr::Add,
        Binary::Multiply => IndexBinaryKindAttr::Multiply,
        Binary::Divide => IndexBinaryKindAttr::Divide,
        Binary::Remainder => IndexBinaryKindAttr::Remainder,
        Binary::Subtract => return Err(Error::Graph),
    })
}

pub(super) struct LiveEvents {
    context: Context,
    function: FuncOp,
    operations: [Option<Ptr<Operation>>; OPERATIONS],
    count: usize,
    words: [Option<Value>; NODES],
    indices: [Option<Value>; NODES],
    leaves: [(Value, Value); INPUTS],
    constants: [Option<u64>; INPUTS],
    view: Value,
    reads: [Option<SemanticTypedReadOp>; 4],
    lane_precondition: Option<Value>,
    epoch: IrMutationAttemptEpoch,
}

impl LiveEvents {
    pub(super) fn new(
        p: &Formula,
        constants: [Option<u64>; INPUTS],
        f: Charge<'_>,
    ) -> Result<Self> {
        formula::check_guarded_addresses(p, f)?;
        formula::charge(
            f,
            std::mem::size_of::<Self>().div_ceil(std::mem::size_of::<usize>()),
        )?;
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &DialectName::try_new(dialect_kernel::DIALECT_NAME).map_err(|_| Error::Graph)?,
        )
        .map_err(|_| Error::Graph)?;
        let function_type = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "bf16_source_events".try_into().map_err(|_| Error::Graph)?,
            function_type,
        );
        formula::charge(f, INPUTS * 2 + 3)?;
        let mut leaves = Vec::with_capacity(INPUTS);
        for (slot, constant) in constants.iter().enumerate() {
            formula::charge(f, 16)?;
            let index = if let Some(value) = constant {
                let op = IndexConstantOp::new(&mut context, *value);
                op.get_operation()
                    .insert_at_back(function.get_entry_block(&context), &context);
                op.result(&context)
            } else {
                let op = IndexUnknownOp::new(&mut context);
                op.get_operation()
                    .insert_at_back(function.get_entry_block(&context), &context);
                op.result(&context)
            };
            let word = if let Some(value) = constant {
                let op = SemanticTypedConstantOp::new(&mut context, *value, scalar(64));
                op.get_operation()
                    .insert_at_back(function.get_entry_block(&context), &context);
                op.result(&context)
            } else {
                let op = SemanticTypedSymbolOp::new(&mut context, slot as u32, scalar(64));
                op.get_operation()
                    .insert_at_back(function.get_entry_block(&context), &context);
                op.result(&context)
            };
            leaves.push((index, word));
        }
        let leaves: [(Value, Value); INPUTS] = leaves.try_into().map_err(|_| Error::Graph)?;
        formula::charge(f, 16)?;
        // No allocation/noalias identifier is fabricated. This graph is relative
        // to its separately retained, exact Global source and physical extent.
        let ty = RankedViewType::new(&mut context, 16, false, vec![DYNAMIC_EXTENT])
            .map_err(|_| Error::Graph)?;
        let view_op = RankedViewOp::new_in_space(
            &mut context,
            ty,
            vec![leaves[7].0],
            MemorySpaceAttr::Global,
        )
        .map_err(|_| Error::Graph)?;
        view_op
            .get_operation()
            .insert_at_back(function.get_entry_block(&context), &context);
        let view = view_op.result(&context);
        let epoch = context
            .ir_mutation_attempt_epoch()
            .map_err(|_| Error::Graph)?;
        let mut result = Self {
            context,
            function,
            leaves,
            constants,
            view,
            epoch,
            operations: [None; OPERATIONS],
            count: 0,
            words: [None; NODES],
            indices: [None; NODES],
            reads: [None; 4],
            lane_precondition: None,
        };
        for (index, (iv, wv)) in result.leaves.iter().copied().enumerate() {
            result.words[index] = Some(wv);
            result.indices[index] = Some(iv);
        }
        // Retained as an explicit pending numerical obligation. A typed lane
        // identity alone is not a proof of its final numeric realization.
        result.lane_precondition = Some(result.emit_word(p, p.precondition, f)?);
        for component in 0..4 {
            let event = p.events[component];
            let index = result.emit_index(p, event.index, f)?;
            let guard = result.emit_word(p, event.guard, f)?;
            formula::charge(f, 24)?;
            let zero = SemanticTypedConstantOp::new(&mut result.context, 0, scalar(16));
            result.append(&zero);
            let fallback = zero.result(&result.context);
            let read = SemanticTypedReadOp::new(
                &mut result.context,
                SEMANTIC_TYPED_READ_SYMBOL_BASE_V1 + component as u32,
                scalar(16),
                MemorySpaceAttr::Global,
                SemanticReadVolatilityAttr::Volatile,
                SemanticReadOrderingAttr::Unordered,
                view,
                vec![index],
                Some((guard, fallback)),
            )
            .map_err(|_| Error::Graph)?;
            result.append(&read);
            result.reads[component] = Some(read);
        }
        formula::charge(f, 8)?;
        let ret = ReturnOp::new(&mut result.context);
        result.append(&ret);
        for pointer in result
            .function
            .get_entry_block(&result.context)
            .deref(&result.context)
            .iter(&result.context)
        {
            formula::charge(f, 1)?;
            if result.count == OPERATIONS {
                return Err(Error::Capacity);
            }
            result.operations[result.count] = Some(pointer);
            result.count += 1;
        }
        result.epoch = result
            .context
            .ir_mutation_attempt_epoch()
            .map_err(|_| Error::Graph)?;
        Ok(result)
    }
    fn append(&self, op: &impl Op) {
        op.get_operation()
            .insert_at_back(self.function.get_entry_block(&self.context), &self.context);
    }
    fn emit_word(&mut self, p: &Formula, id: Id, f: Charge<'_>) -> Result<Value> {
        formula::charge(f, 1)?;
        if let Some(value) = self.words.get(id as usize).copied().flatten() {
            return Ok(value);
        }
        formula::charge(f, 12)?;
        let value = match p.node(id)? {
            Node::Input(_) => return Err(Error::Graph),
            Node::Word(value) => {
                let op = SemanticTypedConstantOp::new(&mut self.context, value, scalar(64));
                self.append(&op);
                op.result(&self.context)
            }
            Node::Bool(value) => {
                let op =
                    SemanticTypedConstantOp::new(&mut self.context, u64::from(value), boolean());
                self.append(&op);
                op.result(&self.context)
            }
            Node::Binary(kind, a, b) => {
                let a = self.emit_word(p, a, f)?;
                let b = self.emit_word(p, b, f)?;
                let op = SemanticTypedBinaryOp::new(
                    &mut self.context,
                    binary(kind),
                    SemanticOverflowAttr::Wrapping,
                    scalar(64),
                    a,
                    b,
                );
                self.append(&op);
                op.result(&self.context)
            }
            Node::Compare(kind, a, b) => {
                let a = self.emit_word(p, a, f)?;
                let b = self.emit_word(p, b, f)?;
                let op =
                    SemanticTypedCompareOp::new(&mut self.context, compare(kind), scalar(64), a, b);
                self.append(&op);
                op.result(&self.context)
            }
            Node::Select {
                boolean: is_bool,
                condition,
                yes,
                no,
            } => {
                let condition = self.emit_word(p, condition, f)?;
                let yes = self.emit_word(p, yes, f)?;
                let no = self.emit_word(p, no, f)?;
                let op = SemanticTypedSelectOp::new(
                    &mut self.context,
                    if is_bool { boolean() } else { scalar(64) },
                    condition,
                    yes,
                    no,
                );
                self.append(&op);
                op.result(&self.context)
            }
        };
        *self.words.get_mut(id as usize).ok_or(Error::Graph)? = Some(value);
        Ok(value)
    }
    fn emit_index(&mut self, p: &Formula, id: Id, f: Charge<'_>) -> Result<Value> {
        formula::charge(f, 1)?;
        if let Some(value) = self.indices.get(id as usize).copied().flatten() {
            return Ok(value);
        }
        formula::charge(f, 10)?;
        let value = match p.node(id)? {
            Node::Word(value) => {
                let op = IndexConstantOp::new(&mut self.context, value);
                self.append(&op);
                op.result(&self.context)
            }
            Node::Binary(kind, a, b) => {
                let a = self.emit_index(p, a, f)?;
                let b = self.emit_index(p, b, f)?;
                let op = IndexBinaryOp::new(&mut self.context, index_binary(kind)?, a, b);
                self.append(&op);
                op.result(&self.context)
            }
            _ => return Err(Error::Graph),
        };
        *self.indices.get_mut(id as usize).ok_or(Error::Graph)? = Some(value);
        Ok(value)
    }
    pub(super) fn verify(&self, expected: &Formula, f: Charge<'_>) -> Result<()> {
        formula::check_guarded_addresses(expected, f)?;
        if self
            .context
            .ir_mutation_attempt_epoch()
            .map_err(|_| Error::Graph)?
            != self.epoch
        {
            return Err(Error::Changed);
        }
        let mut blocks = self
            .function
            .get_region(&self.context)
            .deref(&self.context)
            .iter(&self.context);
        if blocks.next() != Some(self.function.get_entry_block(&self.context))
            || blocks.next().is_some()
        {
            return Err(Error::Graph);
        }
        let mut count = 0;
        let mut reads = 0;
        for pointer in self
            .function
            .get_entry_block(&self.context)
            .deref(&self.context)
            .iter(&self.context)
        {
            formula::charge(f, 1)?;
            if count >= self.count || self.operations[count] != Some(pointer) {
                return Err(Error::Graph);
            }
            count += 1;
            let op = Operation::get_op_dyn(pointer, &self.context);
            op.verify(&self.context).map_err(|_| Error::Graph)?;
            if let Some(read) = op.downcast_ref::<SemanticTypedReadOp>() {
                if reads >= 4 || self.reads[reads].as_ref().map(Op::get_operation) != Some(pointer)
                {
                    return Err(Error::Roster);
                }
                self.check_read(expected, reads, read, f)?;
                reads += 1;
            }
        }
        if count != self.count || reads != 4 {
            return Err(Error::Roster);
        }
        self.matches(
            expected,
            expected.precondition,
            self.lane_precondition.ok_or(Error::Changed)?,
            false,
            0,
            f,
        )?;
        for slot in 0..INPUTS {
            self.check_leaf(slot)?;
        }
        let def =
            Operation::get_op_dyn(self.view.defining_op().ok_or(Error::Graph)?, &self.context);
        let view = def.downcast_ref::<RankedViewOp>().ok_or(Error::Graph)?;
        if view.memory_space(&self.context) != Some(MemorySpaceAttr::Global)
            || view.dynamic_extent(&self.context, 0) != Some(self.leaves[7].0)
            || view.view_type(&self.context).is_none_or(|t| {
                t.deref(&self.context).writable()
                    || t.deref(&self.context).element_width() != 16
                    || t.deref(&self.context).shape() != [DYNAMIC_EXTENT]
            })
        {
            return Err(Error::Changed);
        }
        Ok(())
    }
    fn check_leaf(&self, slot: usize) -> Result<()> {
        let (index, word) = self.leaves[slot];
        let i = Operation::get_op_dyn(index.defining_op().ok_or(Error::Graph)?, &self.context);
        let w = Operation::get_op_dyn(word.defining_op().ok_or(Error::Graph)?, &self.context);
        let valid = match self.constants[slot] {
            Some(value) => {
                i.downcast_ref::<IndexConstantOp>()
                    .is_some_and(|o| o.value(&self.context) == Some(value))
                    && w.downcast_ref::<SemanticTypedConstantOp>()
                        .is_some_and(|o| {
                            o.bits(&self.context) == Some(value)
                                && o.scalar(&self.context) == Some(scalar(64))
                        })
            }
            None => {
                i.downcast_ref::<IndexUnknownOp>().is_some()
                    && w.downcast_ref::<SemanticTypedSymbolOp>().is_some_and(|o| {
                        o.symbol(&self.context) == Some(slot as u32)
                            && o.scalar(&self.context) == Some(scalar(64))
                    })
            }
        };
        if valid { Ok(()) } else { Err(Error::Changed) }
    }
    fn check_read(
        &self,
        p: &Formula,
        component: usize,
        read: &SemanticTypedReadOp,
        f: Charge<'_>,
    ) -> Result<()> {
        if read.symbol(&self.context) != Some(SEMANTIC_TYPED_READ_SYMBOL_BASE_V1 + component as u32)
            || read.scalar(&self.context) != Some(scalar(16))
            || read.view(&self.context) != self.view
            || read.volatility(&self.context) != Some(SemanticReadVolatilityAttr::Volatile)
            || read.ordering(&self.context) != Some(SemanticReadOrderingAttr::Unordered)
            || read.memory_space(&self.context) != Some(MemorySpaceAttr::Global)
            || read.result(&self.context).defining_op() != Some(read.get_operation())
        {
            return Err(Error::Changed);
        }
        let indices = read.indices(&self.context).ok_or(Error::Graph)?;
        if indices.len() != 1 {
            return Err(Error::Graph);
        }
        let (guard, fallback) = read.guarded(&self.context).ok_or(Error::Changed)?;
        let zero =
            Operation::get_op_dyn(fallback.defining_op().ok_or(Error::Graph)?, &self.context);
        if zero
            .downcast_ref::<SemanticTypedConstantOp>()
            .is_none_or(|o| {
                o.scalar(&self.context) != Some(scalar(16)) || o.bits(&self.context) != Some(0)
            })
        {
            return Err(Error::Changed);
        }
        self.matches(p, p.events[component].index, indices[0], true, 0, f)?;
        self.matches(p, p.events[component].guard, guard, false, 0, f)
    }
    fn matches(
        &self,
        p: &Formula,
        id: Id,
        value: Value,
        index: bool,
        depth: usize,
        f: Charge<'_>,
    ) -> Result<()> {
        formula::charge(f, 1)?;
        if depth > 64 {
            return Err(Error::Capacity);
        }
        let node = p.node(id)?;
        if let Node::Input(slot) = node {
            return if self
                .leaves
                .get(slot as usize)
                .map(|v| if index { v.0 } else { v.1 })
                == Some(value)
            {
                Ok(())
            } else {
                Err(Error::Changed)
            };
        }
        let op = Operation::get_op_dyn(value.defining_op().ok_or(Error::Graph)?, &self.context);
        let raw = op.get_operation().deref(&self.context);
        if raw.get_num_results() != 1 || raw.get_result(0) != value {
            return Err(Error::Changed);
        }
        let next = depth + 1;
        match node {
            Node::Word(expected) if index => {
                if op
                    .downcast_ref::<IndexConstantOp>()
                    .is_none_or(|o| o.value(&self.context) != Some(expected))
                {
                    return Err(Error::Changed);
                }
            }
            Node::Word(expected) => {
                if op
                    .downcast_ref::<SemanticTypedConstantOp>()
                    .is_none_or(|o| {
                        o.bits(&self.context) != Some(expected)
                            || o.scalar(&self.context) != Some(scalar(64))
                    })
                {
                    return Err(Error::Changed);
                }
            }
            Node::Bool(expected) if !index => {
                if op
                    .downcast_ref::<SemanticTypedConstantOp>()
                    .is_none_or(|o| {
                        o.bits(&self.context) != Some(u64::from(expected))
                            || o.scalar(&self.context) != Some(boolean())
                    })
                {
                    return Err(Error::Changed);
                }
            }
            Node::Binary(kind, a, b) if index => {
                let o = op.downcast_ref::<IndexBinaryOp>().ok_or(Error::Changed)?;
                if o.kind(&self.context) != Some(index_binary(kind)?) {
                    return Err(Error::Changed);
                }
                self.matches(p, a, o.lhs(&self.context), true, next, f)?;
                self.matches(p, b, o.rhs(&self.context), true, next, f)?;
            }
            Node::Binary(kind, a, b) => {
                let o = op
                    .downcast_ref::<SemanticTypedBinaryOp>()
                    .ok_or(Error::Changed)?;
                if o.kind(&self.context) != Some(binary(kind))
                    || o.scalar(&self.context) != Some(scalar(64))
                    || o.overflow(&self.context) != Some(SemanticOverflowAttr::Wrapping)
                {
                    return Err(Error::Changed);
                }
                self.matches(p, a, o.lhs(&self.context), false, next, f)?;
                self.matches(p, b, o.rhs(&self.context), false, next, f)?;
            }
            Node::Compare(kind, a, b) if !index => {
                let o = op
                    .downcast_ref::<SemanticTypedCompareOp>()
                    .ok_or(Error::Changed)?;
                if o.kind(&self.context) != Some(compare(kind))
                    || o.operand_scalar(&self.context) != Some(scalar(64))
                {
                    return Err(Error::Changed);
                }
                self.matches(p, a, o.lhs(&self.context), false, next, f)?;
                self.matches(p, b, o.rhs(&self.context), false, next, f)?;
            }
            Node::Select {
                boolean: is_bool,
                condition,
                yes,
                no,
            } if !index => {
                let o = op
                    .downcast_ref::<SemanticTypedSelectOp>()
                    .ok_or(Error::Changed)?;
                if o.scalar(&self.context) != Some(if is_bool { boolean() } else { scalar(64) }) {
                    return Err(Error::Changed);
                }
                self.matches(p, condition, o.condition(&self.context), false, next, f)?;
                self.matches(p, yes, o.when_true(&self.context), false, next, f)?;
                self.matches(p, no, o.when_false(&self.context), false, next, f)?;
            }
            _ => return Err(Error::Changed),
        }
        Ok(())
    }
    pub(super) fn results(&self) -> Result<[Value; 4]> {
        let mut values = [None; 4];
        for (slot, read) in self.reads.iter().enumerate() {
            values[slot] = Some(read.as_ref().ok_or(Error::Roster)?.result(&self.context));
        }
        Ok(values.map(|v| v.unwrap()))
    }
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
