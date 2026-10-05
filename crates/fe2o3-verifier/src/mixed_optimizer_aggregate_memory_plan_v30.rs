//! Original-derived memory cut requirements. This is proof dependency closure,
//! not phi placement or another SSA construction algorithm.
use super::*;

pub(super) struct MemoryPlan {
    pub output_origins: Vec<usize>,
    pub output_sources: Vec<usize>,
    pub parameter_definitions: Vec<usize>,
    pub requirements: Vec<usize>,
    pub last_write: Vec<usize>,
    pub slots: usize,
    pub source_values: usize,
    predecessors: Vec<usize>,
    next: Vec<usize>,
    edge_sources: Vec<usize>,
    pending: Vec<usize>,
    tail: usize,
}
const RESET: usize = usize::MAX - 1;
pub(super) const TRUE: usize = usize::MAX - 1;

pub(super) fn value_index(
    inv: &Inventory<'_>,
    function: usize,
    value: fe2o3_kernel_ir::ValueId,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    let function = inv
        .functions()
        .get(function)
        .ok_or(Error::Statement("aggregate value function"))?;
    for definition in function.definitions.clone() {
        out.budget.charge_work(1)?;
        if inv.definitions()[definition].value == Some(value) {
            return Ok(definition);
        }
    }
    Err(Error::Statement("aggregate exact actual SSA value"))
}
fn function(inv: &Inventory<'_>, block: usize) -> usize {
    inv.blocks()[block].coordinate.function.0 as usize
}

impl MemoryPlan {
    pub fn build(
        input: &Inventory<'_>,
        output: &Inventory<'_>,
        witness: &Witness,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        if input.blocks().len() != output.blocks().len()
            || witness.actions().len() != input.operations().len()
            || witness.memory_events().len() != input.operations().len()
            || witness.parameters().len() != witness.memory_parameters().len()
            || witness.conditions().len() != input.functions().len()
        {
            return Err(Error::Statement(
                "complete aggregate graph and concrete memory roster",
            ));
        }
        let slots = witness.memory_slots().len();
        let cells = input
            .blocks()
            .len()
            .checked_mul(slots)
            .ok_or(Resource::Arithmetic)?;
        out.budget
            .reserve_storage(size_of::<Self>() + size_of::<Result<Self>>())?;
        let mut plan = Self {
            output_origins: allocate(output.operations().len(), out)?,
            output_sources: allocate(output.definitions().len(), out)?,
            parameter_definitions: allocate(witness.parameters().len(), out)?,
            requirements: allocate(cells, out)?,
            last_write: allocate(cells, out)?,
            slots,
            source_values: input
                .definitions()
                .len()
                .checked_add(witness.parameters().len())
                .ok_or(Resource::Arithmetic)?,
            predecessors: allocate(input.blocks().len(), out)?,
            next: allocate(input.edges().len(), out)?,
            edge_sources: allocate(input.edges().len(), out)?,
            pending: allocate(cells, out)?,
            tail: 0,
        };
        for (b, (a, z)) in input.blocks().iter().zip(output.blocks()).enumerate() {
            out.budget.charge_work(3)?;
            if a.coordinate != z.coordinate || a.edges.len() != z.edges.len() {
                return Err(Error::Statement(
                    "aggregate unchanged CFG coordinate roster",
                ));
            }
            let mut target = z.operations.start;
            if a.coordinate.block == 0 && witness.conditions()[function(input, b)].is_some() {
                target += 1;
            }
            for op in a.operations.clone() {
                out.budget.charge_work(1)?;
                if witness.actions()[op] != Action::Remove {
                    if target >= z.operations.end {
                        return Err(Error::Statement("aggregate retained operation ordinal"));
                    }
                    plan.output_origins[target] = op;
                    target += 1;
                }
            }
            if target != z.operations.end {
                return Err(Error::Statement(
                    "aggregate complete retained operation map",
                ));
            }
            for edge in a.edges.clone() {
                out.budget.charge_work(4)?;
                let destination = block_index(input, input.edges()[edge].target)?;
                plan.next[edge] = plan.predecessors[destination];
                plan.predecessors[destination] = edge;
                plan.edge_sources[edge] = b;
            }
        }
        for link in witness.memory_parameters() {
            out.budget.charge_work(3)?;
            let parameter = witness
                .parameters()
                .get(link.parameter)
                .ok_or(Error::Statement("aggregate phi row index"))?;
            if plan.parameter_definitions[link.parameter] != NONE {
                return Err(Error::Statement("aggregate unique phi slot link"));
            }
            let definition = value_index(
                output,
                function(output, parameter.block),
                parameter.value,
                out,
            )?;
            if !output.blocks()[parameter.block]
                .parameters
                .contains(&definition)
            {
                return Err(Error::Statement("aggregate actual appended parameter"));
            }
            plan.parameter_definitions[link.parameter] = definition;
            plan.output_sources[definition] = input.definitions().len() + link.parameter;
            plan.require(
                input,
                output,
                witness,
                parameter.block,
                link.slot,
                definition,
                out,
            )?;
        }
        for (f, row) in output.functions().iter().enumerate() {
            if row.blocks.is_empty() {
                continue;
            }
            for d in row.definitions.clone() {
                if plan.output_sources[d] != NONE {
                    continue;
                }
                out.budget.charge_work(2)?;
                let value = output.definitions()[d]
                    .value
                    .ok_or(Error::Statement("aggregate actual definition identity"))?;
                if witness.conditions()[f] == Some(value) {
                    plan.output_sources[d] = TRUE;
                    continue;
                }
                plan.output_sources[d] = value_index(input, f, value, out)?;
            }
        }
        for (b, block) in input.blocks().iter().enumerate() {
            for op in block.operations.clone() {
                out.budget.charge_work(2)?;
                match witness.memory_events()[op] {
                    Memory::Allocate { allocation } => {
                        for (slot, row) in witness.memory_slots().iter().enumerate() {
                            out.budget.charge_work(1)?;
                            if row.is_some_and(|row| row.allocation == allocation) {
                                plan.last_write[b * slots + slot] = RESET;
                            }
                        }
                    }
                    Memory::Write { slot, .. } => plan.last_write[b * slots + slot] = op,
                    Memory::Read {
                        slot, replacement, ..
                    } => {
                        let value = value_index(output, function(input, b), replacement, out)?;
                        plan.at_end(input, output, witness, b, slot, value, out)?;
                    }
                    _ => {}
                }
            }
        }
        let mut head = 0;
        while head < plan.tail {
            out.budget.charge_work(3)?;
            let cell = plan.pending[head];
            head += 1;
            let block = cell / slots;
            let slot = cell % slots;
            let needed = plan.requirements[cell];
            let mut edge = plan.predecessors[block];
            while edge != NONE {
                out.budget.charge_work(3)?;
                let source = plan.edge_sources[edge];
                let translated = plan.edge_value(output, edge, needed, out)?;
                plan.at_end(input, output, witness, source, slot, translated, out)?;
                edge = plan.next[edge];
            }
            // Selected cells cannot be live before their original allocation.
            if input.blocks()[block].coordinate.block == 0 {
                return Err(Error::Statement(
                    "aggregate memory requirement before entry allocation",
                ));
            }
        }
        plan.validate(input, output, witness, out)?;
        Ok(plan)
    }
    fn normalized(
        &self,
        output: &Inventory<'_>,
        witness: &Witness,
        block: usize,
        mut value: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<usize> {
        for _ in 0..=output.definitions().len() {
            out.budget.charge_work(2)?;
            if !local(output, value, block) {
                return Ok(value);
            }
            let Definition::Result { operation, .. } = output.definitions()[value].coordinate
            else {
                unreachable!()
            };
            let original = self.output_origins[operation_index(output, operation)?];
            let Some(Action::Copy(next)) = witness.actions().get(original) else {
                return Ok(value);
            };
            value = value_index(output, function(output, block), *next, out)?;
        }
        Err(Error::Statement("aggregate copied-value dependency cycle"))
    }
    fn require(
        &mut self,
        _input: &Inventory<'_>,
        output: &Inventory<'_>,
        witness: &Witness,
        block: usize,
        slot: usize,
        value: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        if witness
            .memory_slots()
            .get(slot)
            .is_none_or(|slot| slot.is_none())
        {
            return Err(Error::Statement(
                "aggregate requirement names selected slot",
            ));
        }
        let value = self.normalized(output, witness, block, value, out)?;
        if local(output, value, block) {
            return Err(Error::Statement(
                "aggregate entry memory cannot depend on later opaque value",
            ));
        }
        let cell = block * self.slots + slot;
        out.budget.charge_work(2)?;
        if self.requirements[cell] == NONE {
            self.requirements[cell] = value;
            *self
                .pending
                .get_mut(self.tail)
                .ok_or(Error::Statement("aggregate requirement queue census"))? = cell;
            self.tail += 1;
        } else if self.requirements[cell] != value {
            return Err(Error::Statement("aggregate unique memory value at cut"));
        }
        Ok(())
    }
    fn at_end(
        &mut self,
        input: &Inventory<'_>,
        output: &Inventory<'_>,
        witness: &Witness,
        block: usize,
        slot: usize,
        value: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let last = self.last_write[block * self.slots + slot];
        if last == NONE {
            return self.require(input, output, witness, block, slot, value, out);
        }
        if last == RESET {
            return Err(Error::Statement(
                "aggregate memory use after uninitialized allocation reset",
            ));
        }
        let Memory::Write { value: stored, .. } = witness.memory_events()[last] else {
            return Err(Error::Statement("aggregate last write event"));
        };
        let stored = value_index(output, function(output, block), stored, out)?;
        // A stored local non-copy value is a genuine initialized SSA value, so
        // equality may be direct before applying copy normalization.
        if stored == value {
            return Ok(());
        }
        if self.normalized(output, witness, block, stored, out)?
            != self.normalized(output, witness, block, value, out)?
        {
            return Err(Error::Statement("aggregate actual last-store value"));
        }
        Ok(())
    }
    pub fn edge_value(
        &self,
        output: &Inventory<'_>,
        edge: usize,
        value: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<usize> {
        let row = &output.edges()[edge];
        let target = block_index(output, row.target)?;
        let parameters = &output.blocks()[target].parameters;
        if parameters.contains(&value) {
            let ordinal = value - parameters.start;
            out.budget.charge_work(2)?;
            return Ok(output.edge_arguments()[row.bindings.start + ordinal].incoming_definition);
        }
        Ok(value)
    }
    pub(super) fn validate(
        &self,
        input: &Inventory<'_>,
        output: &Inventory<'_>,
        witness: &Witness,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let mut writes = allocate(self.slots, out)?;
        for link in witness.memory_parameters() {
            out.budget.charge_work(2)?;
            let block = witness.parameters()[link.parameter].block;
            if self.requirements[block * self.slots + link.slot]
                != self.parameter_definitions[link.parameter]
            {
                return Err(Error::Statement(
                    "aggregate closure omits actual phi memory",
                ));
            }
        }
        for (block, row) in input.blocks().iter().enumerate() {
            out.budget.charge_work(self.slots)?;
            writes.fill(NONE);
            // Independently replay the original event order. The queue's final
            // edge closure is insufficient if a local read seed was omitted.
            for operation in row.operations.clone() {
                out.budget.charge_work(2)?;
                match witness.memory_events()[operation] {
                    Memory::Allocate { allocation } => {
                        for (slot, selected) in witness.memory_slots().iter().enumerate() {
                            out.budget.charge_work(1)?;
                            if selected.is_some_and(|row| row.allocation == allocation) {
                                writes[slot] = RESET;
                            }
                        }
                    }
                    Memory::Write { slot, .. } => writes[slot] = operation,
                    Memory::Read {
                        slot, replacement, ..
                    } => {
                        let needed =
                            value_index(output, function(output, block), replacement, out)?;
                        let last = writes[slot];
                        let expected = if last == NONE {
                            self.requirements[block * self.slots + slot]
                        } else if last == RESET {
                            return Err(Error::Statement(
                                "aggregate read follows allocation reset",
                            ));
                        } else {
                            let Memory::Write { value, .. } = witness.memory_events()[last] else {
                                return Err(Error::Statement("aggregate read last-write event"));
                            };
                            value_index(output, function(output, block), value, out)?
                        };
                        if expected == NONE
                            || self.normalized(output, witness, block, expected, out)?
                                != self.normalized(output, witness, block, needed, out)?
                        {
                            return Err(Error::Statement(
                                "aggregate closure omits original read memory",
                            ));
                        }
                    }
                    _ => {}
                }
            }
            for slot in 0..self.slots {
                out.budget.charge_work(1)?;
                if writes[slot] != self.last_write[block * self.slots + slot] {
                    return Err(Error::Statement("aggregate exact final write or reset"));
                }
                let needed = self.requirements[block * self.slots + slot];
                if needed == NONE {
                    continue;
                }
                if row.coordinate.block == 0
                    || witness.memory_slots()[slot].is_none()
                    || local(output, needed, block)
                {
                    return Err(Error::Statement("aggregate closed entry requirement"));
                }
                let mut edge = self.predecessors[block];
                while edge != NONE {
                    out.budget.charge_work(3)?;
                    let source = self.edge_sources[edge];
                    let incoming = self.edge_value(output, edge, needed, out)?;
                    let last = self.last_write[source * self.slots + slot];
                    let expected = if last == NONE {
                        self.requirements[source * self.slots + slot]
                    } else if last == RESET {
                        return Err(Error::Statement("aggregate initialized edge requirement"));
                    } else {
                        let Memory::Write { value, .. } = witness.memory_events()[last] else {
                            return Err(Error::Statement("aggregate edge write event"));
                        };
                        value_index(output, function(output, source), value, out)?
                    };
                    if expected == NONE
                        || (incoming != expected
                            && self.normalized(output, witness, source, incoming, out)?
                                != self.normalized(output, witness, source, expected, out)?)
                    {
                        return Err(Error::Statement(
                            "aggregate complete exact edge memory closure",
                        ));
                    }
                    edge = self.next[edge];
                }
            }
        }
        Ok(())
    }
}
