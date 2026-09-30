// Reconstruct live-in names from the existing source SSA plan's checked events
// and the caller's complete original ordered edge roster.
use fe2o3_mir_model::{
    SsaArgumentV1, SsaBlockIdV1 as Block, SsaConstructionPlanV1 as Plan, SsaEdgeIdV1 as Edge,
    SsaResolvedEventV1 as Event, SsaValueV1 as Value, SsaVariableIdV1 as Variable,
};
use std::mem::size_of;

const MAX_BOUNDARY_ROWS: usize = fe2o3_mir_model::HARD_MAX_SSA_OUTPUT_ITEMS_V1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Row {
    block: Block,
    variable: Variable,
    parent: usize,
    rank: u32,
    value: Option<Value>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Resolved {
    block: Block,
    variable: Variable,
    ordinal: u32,
    event: Event,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Current {
    Entry(usize),
    Value(Value),
    Untracked,
    Undefined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Exit {
    block: Block,
    variable: Variable,
    current: Current,
}

/// The complete successor roster must be rederived from the unchanged source.
/// Edge definitions override only their exact source-edge variable.
struct ControlInput<'a> {
    pub entry: Block,
    pub successors: &'a [Vec<Block>],
}

struct Boundaries {
    rows: Vec<Row>,
}

fn malformed() -> Error {
    Error::Statement("original MIR SSA boundary equations differ")
}

fn logarithm(count: usize) -> usize {
    (usize::BITS - count.max(1).leading_zeros()) as usize + 1
}

fn headers() -> usize {
    size_of::<Boundaries>()
        + size_of::<Result<Boundaries>>()
        + size_of::<Vec<Resolved>>()
        + size_of::<Vec<Exit>>()
        + size_of::<ControlInput<'_>>()
        + size_of::<Row>()
        + size_of::<Resolved>()
        + size_of::<Exit>()
        + size_of::<Current>()
        + size_of::<Option<usize>>()
        + size_of::<Option<Value>>()
        + size_of::<Result<()>>()
        + size_of::<&Plan>()
        + size_of::<&mut Meter<'_, '_>>()
        + size_of::<std::slice::Iter<'_, Vec<Block>>>()
        + size_of::<std::slice::Iter<'_, Block>>()
        + size_of::<std::slice::Iter<'_, (u32, Event)>>()
        + size_of::<std::slice::Iter<'_, SsaArgumentV1>>()
        + size_of::<Result<Vec<Row>>>()
        + size_of::<Result<Vec<Resolved>>>()
        + size_of::<Result<Vec<Exit>>>()
        + size_of::<Result<(Vec<Row>, usize)>>()
        + size_of::<Result<(Vec<Resolved>, usize)>>()
        + size_of::<Result<(Vec<Exit>, usize)>>()
}

impl Boundaries {
    fn derive(plan: &Plan, input: ControlInput<'_>, out: &mut Meter<'_, '_>) -> Result<Self> {
        out.reserve(headers())?;
        if input.successors.len() != plan.resources().input_blocks()
            || !plan.is_reachable(input.entry)
        {
            return Err(malformed());
        }
        let mut count = 0usize;
        let mut event_count = 0usize;
        for block in 0..input.successors.len() {
            out.work(3)?;
            let block = Block::new(u32::try_from(block).map_err(|_| Resource::Arithmetic)?);
            if let Some(live) = plan.live_in(block) {
                count = count.checked_add(live.len()).ok_or(Resource::Arithmetic)?;
                event_count = event_count
                    .checked_add(plan.resolved_events(block).ok_or_else(malformed)?.len())
                    .ok_or(Resource::Arithmetic)?;
            }
        }
        if count > MAX_BOUNDARY_ROWS || event_count > MAX_BOUNDARY_ROWS {
            return Err(Error::Statement(
                "original MIR SSA boundary census exceeds bound",
            ));
        }
        let mut result = Self {
            rows: vector(count, out)?,
        };
        let mut events: Vec<Resolved> = vector(event_count, out)?;
        let mut exits: Vec<Exit> = vector(event_count, out)?;
        for block in 0..input.successors.len() {
            out.work(3)?;
            let block = Block::new(u32::try_from(block).map_err(|_| Resource::Arithmetic)?);
            let Some(live) = plan.live_in(block) else {
                continue;
            };
            let mut previous = None;
            for &variable in live {
                out.work(2)?;
                if previous.is_some_and(|last| last >= variable) {
                    return Err(malformed());
                }
                previous = Some(variable);
                let parent = result.rows.len();
                result.rows.push(Row {
                    block,
                    variable,
                    parent,
                    rank: 0,
                    value: None,
                });
            }
            for &(ordinal, event) in plan.resolved_events(block).ok_or_else(malformed)? {
                out.work(2)?;
                let variable = match event {
                    Event::Use { variable, .. }
                    | Event::Define { variable, .. }
                    | Event::Kill { variable, .. } => variable,
                };
                events.push(Resolved {
                    block,
                    variable,
                    ordinal,
                    event,
                });
            }
        }
        out.work(
            event_count
                .checked_mul(logarithm(event_count) + 2)
                .ok_or(Resource::Arithmetic)?,
        )?;
        events.sort_unstable_by_key(|row| (row.block, row.variable, row.ordinal));
        let mut at = 0usize;
        while at < events.len() {
            let first = events[at];
            let mut current = result
                .index(first.block, first.variable, out)?
                .map(Current::Entry)
                .unwrap_or(Current::Untracked);
            let mut previous = None;
            while at < events.len()
                && (events[at].block, events[at].variable) == (first.block, first.variable)
            {
                out.work(4)?;
                let event = events[at];
                if previous.is_some_and(|last| last >= event.ordinal) {
                    return Err(malformed());
                }
                previous = Some(event.ordinal);
                match event.event {
                    Event::Use { value, .. } => result.expect(current, Some(value), out)?,
                    Event::Define { value, .. } => current = Current::Value(value),
                    Event::Kill { previous, .. } => {
                        // A dead value may still have a planner name. Killing
                        // it does not make that value live at the block entry.
                        if current != Current::Untracked {
                            result.expect(current, previous, out)?;
                        }
                        current = Current::Undefined;
                    }
                }
                at += 1;
            }
            exits.push(Exit {
                block: first.block,
                variable: first.variable,
                current,
            });
        }
        for block in 0..input.successors.len() {
            out.work(2)?;
            let block = Block::new(u32::try_from(block).map_err(|_| Resource::Arithmetic)?);
            let Some(transport) = plan.transport_variables(block) else {
                continue;
            };
            for &variable in transport {
                let row = result.index(block, variable, out)?.ok_or_else(malformed)?;
                result.seed(row, Value::BlockArgument { block, variable }, out)?;
            }
        }
        let entry_transport = plan
            .transport_variables(input.entry)
            .ok_or_else(malformed)?;
        if plan.entry_arguments().len() != entry_transport.len() {
            return Err(malformed());
        }
        for (argument, variable) in plan.entry_arguments().iter().zip(entry_transport) {
            out.work(2)?;
            if argument.variable() != *variable
                || Self::argument(plan.entry_definitions(), *variable, out)?
                    != Some(argument.value())
            {
                return Err(malformed());
            }
        }
        for &variable in plan.live_in(input.entry).ok_or_else(malformed)? {
            out.work(logarithm(entry_transport.len()))?;
            if entry_transport.binary_search(&variable).is_err() {
                let value = Self::argument(plan.entry_definitions(), variable, out)?
                    .ok_or_else(malformed)?;
                let row = result
                    .index(input.entry, variable, out)?
                    .ok_or_else(malformed)?;
                result.seed(row, value, out)?;
            }
        }
        for (block, successors) in input.successors.iter().enumerate() {
            let block = Block::new(u32::try_from(block).map_err(|_| Resource::Arithmetic)?);
            out.work(2)?;
            if !plan.is_reachable(block) {
                continue;
            }
            for (ordinal, &target) in successors.iter().enumerate() {
                out.work(4)?;
                let edge = Edge::new(
                    block,
                    u32::try_from(ordinal).map_err(|_| Resource::Arithmetic)?,
                );
                let definitions = plan.edge_definitions(edge).ok_or_else(malformed)?;
                let arguments = plan.edge_arguments(edge).ok_or_else(malformed)?;
                let transport = plan.transport_variables(target).ok_or_else(malformed)?;
                if arguments.len() != transport.len() {
                    return Err(malformed());
                }
                let mut previous = None;
                for definition in definitions {
                    out.work(4)?;
                    if previous.is_some_and(|old| old >= definition.variable())
                        || !matches!(definition.value(), Value::Definition(value)
                            if (value.get() as usize) < plan.definition_count())
                    {
                        return Err(malformed());
                    }
                    previous = Some(definition.variable());
                }
                for (argument, &variable) in arguments.iter().zip(transport) {
                    out.work(2)?;
                    if argument.variable() != variable {
                        return Err(malformed());
                    }
                    let current = result.edge_exit(definitions, &exits, block, variable, out)?;
                    result.expect(current, Some(argument.value()), out)?;
                }
                for &variable in plan.live_in(target).ok_or_else(malformed)? {
                    out.work(logarithm(transport.len()))?;
                    if transport.binary_search(&variable).is_ok() {
                        continue;
                    }
                    let target_row = result.index(target, variable, out)?.ok_or_else(malformed)?;
                    match result.edge_exit(definitions, &exits, block, variable, out)? {
                        Current::Entry(source_row) => result.union(source_row, target_row, out)?,
                        Current::Value(value) => result.seed(target_row, value, out)?,
                        Current::Untracked | Current::Undefined => return Err(malformed()),
                    }
                }
            }
            let extra = Edge::new(
                block,
                u32::try_from(successors.len()).map_err(|_| Resource::Arithmetic)?,
            );
            if plan.edge_arguments(extra).is_some() || plan.edge_definitions(extra).is_some() {
                return Err(malformed());
            }
        }
        for row in 0..result.rows.len() {
            let root = result.root(row, out)?;
            let value = result.rows[root].value.ok_or_else(malformed)?;
            result.rows[row].value = Some(value);
        }
        Ok(result)
    }

    fn value(&self, block: Block, variable: Variable, out: &mut Meter<'_, '_>) -> Result<Value> {
        self.index(block, variable, out)?
            .and_then(|at| self.rows[at].value)
            .ok_or_else(malformed)
    }

    fn index(
        &self,
        block: Block,
        variable: Variable,
        out: &mut Meter<'_, '_>,
    ) -> Result<Option<usize>> {
        out.work(logarithm(self.rows.len()))?;
        Ok(self
            .rows
            .binary_search_by_key(&(block, variable), |row| (row.block, row.variable))
            .ok())
    }

    fn argument(
        arguments: &[SsaArgumentV1],
        variable: Variable,
        out: &mut Meter<'_, '_>,
    ) -> Result<Option<Value>> {
        out.work(logarithm(arguments.len()))?;
        Ok(arguments
            .binary_search_by_key(&variable, |argument| argument.variable())
            .ok()
            .map(|at| arguments[at].value()))
    }

    fn exit(
        &self,
        exits: &[Exit],
        block: Block,
        variable: Variable,
        out: &mut Meter<'_, '_>,
    ) -> Result<Current> {
        out.work(logarithm(exits.len()))?;
        if let Ok(at) =
            exits.binary_search_by_key(&(block, variable), |row| (row.block, row.variable))
        {
            return Ok(exits[at].current);
        }
        Ok(self
            .index(block, variable, out)?
            .map(Current::Entry)
            .unwrap_or(Current::Undefined))
    }

    fn edge_exit(
        &self,
        definitions: &[SsaArgumentV1],
        exits: &[Exit],
        block: Block,
        variable: Variable,
        out: &mut Meter<'_, '_>,
    ) -> Result<Current> {
        match Self::argument(definitions, variable, out)? {
            Some(value) => Ok(Current::Value(value)),
            None => self.exit(exits, block, variable, out),
        }
    }

    fn root(&self, mut row: usize, out: &mut Meter<'_, '_>) -> Result<usize> {
        loop {
            out.work(2)?;
            let parent = self.rows.get(row).ok_or_else(malformed)?.parent;
            if parent == row {
                return Ok(row);
            }
            row = parent;
        }
    }

    fn seed(&mut self, row: usize, value: Value, out: &mut Meter<'_, '_>) -> Result<()> {
        let root = self.root(row, out)?;
        if self.rows[root].value.is_some_and(|known| known != value) {
            return Err(malformed());
        }
        self.rows[root].value = Some(value);
        Ok(())
    }

    fn expect(
        &mut self,
        current: Current,
        value: Option<Value>,
        out: &mut Meter<'_, '_>,
    ) -> Result<()> {
        out.work(1)?;
        match (current, value) {
            (Current::Entry(row), Some(value)) => self.seed(row, value, out),
            (Current::Value(actual), Some(expected)) if actual == expected => Ok(()),
            (Current::Undefined, None) => Ok(()),
            _ => Err(malformed()),
        }
    }

    fn union(&mut self, left: usize, right: usize, out: &mut Meter<'_, '_>) -> Result<()> {
        let mut left = self.root(left, out)?;
        let mut right = self.root(right, out)?;
        if left == right {
            return Ok(());
        }
        if let (Some(a), Some(b)) = (self.rows[left].value, self.rows[right].value)
            && a != b
        {
            return Err(malformed());
        }
        if self.rows[left].rank < self.rows[right].rank {
            std::mem::swap(&mut left, &mut right);
        }
        self.rows[right].parent = left;
        self.rows[left].value = self.rows[left].value.or(self.rows[right].value);
        if self.rows[left].rank == self.rows[right].rank {
            self.rows[left].rank = self.rows[left]
                .rank
                .checked_add(1)
                .ok_or(Resource::Arithmetic)?;
        }
        Ok(())
    }
}
