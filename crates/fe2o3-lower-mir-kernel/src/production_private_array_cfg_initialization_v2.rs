//! Sparse per-element source initialization over the genuine source CFG.
//! Addresses and writes still require the independent physical relation.
use super::*;

type Error = ProductionMirPlironTranslationErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Work<'a> = PrivateArrayCorrelationWorkV1<'a>;

fn resource() -> Error {
    Error::ResourceLimit
}
fn mismatch() -> Error {
    Error::KernelShape
}

#[derive(Default)]
struct Scratch {
    bytes: usize,
}
impl Scratch {
    fn reserve(&mut self, bytes: usize, work: &mut Work<'_>) -> Result<()> {
        let total = self.bytes.checked_add(bytes).ok_or_else(resource)?;
        work.budget
            .reserve_private_array_scratch(bytes)
            .ok_or_else(resource)?;
        self.bytes = total;
        Ok(())
    }
    fn vector<T>(&mut self, capacity: usize, work: &mut Work<'_>) -> Result<Vec<T>> {
        work.charge_private_array_work(2)?;
        let bytes = capacity
            .checked_mul(std::mem::size_of::<T>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Vec<T>>()))
            .ok_or_else(resource)?;
        self.reserve(bytes, work)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(capacity).map_err(|_| resource())?;
        let excess = rows
            .capacity()
            .checked_sub(capacity)
            .and_then(|n| n.checked_mul(std::mem::size_of::<T>()))
            .ok_or_else(resource)?;
        self.reserve(excess, work)?;
        Ok(rows)
    }
    fn push<T: Copy>(&mut self, rows: &mut Vec<T>, value: T, work: &mut Work<'_>) -> Result<()> {
        work.charge_private_array_work(2)?;
        if rows.len() == rows.capacity() {
            let capacity = rows.len().checked_mul(2).ok_or_else(resource)?.max(1);
            let mut replacement = self.vector(capacity, work)?;
            work.charge_private_array_work(rows.len())?;
            replacement.extend_from_slice(rows);
            let old = std::mem::replace(rows, replacement);
            let bytes = vector_bytes(&old)?;
            drop(old);
            self.release(bytes, work)?;
        }
        rows.push(value);
        Ok(())
    }
    fn release(&mut self, bytes: usize, work: &mut Work<'_>) -> Result<()> {
        let remaining = self.bytes.checked_sub(bytes).ok_or_else(resource)?;
        work.budget
            .release_private_array_scratch(bytes)
            .ok_or_else(resource)?;
        self.bytes = remaining;
        Ok(())
    }
}
fn vector_bytes<T>(rows: &Vec<T>) -> Result<usize> {
    rows.capacity()
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|n| n.checked_add(std::mem::size_of::<Vec<T>>()))
        .ok_or_else(resource)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CellKey {
    local: u32,
    offset: u64,
}
impl CellKey {
    fn key(self) -> [usize; 2] {
        [self.local as usize, self.offset as usize]
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Read(usize),
    Initialize,
    Kill,
}
#[derive(Clone, Copy)]
struct Event {
    cell: CellKey,
    block: usize,
    statement: usize,
    operation: usize,
    phase: usize,
    action: Action,
}
impl Event {
    fn time(self) -> [usize; 4] {
        [self.block, self.statement, self.operation, self.phase]
    }
    fn key(self) -> [usize; 6] {
        [
            self.cell.local as usize,
            self.cell.offset as usize,
            self.block,
            self.statement,
            self.operation,
            self.phase,
        ]
    }
}
#[derive(Clone, Copy)]
struct Kill {
    local: u32,
    block: usize,
    statement: usize,
    operation: usize,
    phase: usize,
}
impl Kill {
    fn time(self) -> [usize; 4] {
        [self.block, self.statement, self.operation, self.phase]
    }
    fn key(self) -> [usize; 5] {
        [
            self.local as usize,
            self.block,
            self.statement,
            self.operation,
            self.phase,
        ]
    }
}

struct Cfg {
    edges: Vec<[usize; 2]>,
    ranges: Vec<(usize, usize)>,
    reachable: Vec<bool>,
    entry: usize,
}

fn check_assert_failure_sink(block: &BasicBlock, work: &mut Work<'_>) -> Result<()> {
    work.charge_private_array_work(8)?;
    if !block.parameters.is_empty()
        || !matches!(block.terminator, Some(Terminator::Unreachable))
        || block.operations.len() != 1
    {
        return Err(mismatch());
    }
    let operation = &block.operations[0];
    let OperationKind::Call { callee, arguments } = &operation.kind else {
        return Err(mismatch());
    };
    if !operation.results.is_empty() || !arguments.is_empty() {
        return Err(mismatch());
    }
    // The existing decoder checks eight fixed descriptors. Empty arguments
    // rule out the allocating Print cases before any operand copy.
    work.charge_private_array_work(
        callee
            .as_str()
            .len()
            .checked_add(2)
            .and_then(|n| n.checked_mul(8))
            .ok_or_else(resource)?,
    )?;
    if !matches!(
        AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments),
        Some(AmdGpuDiagnosticOperation::Trap)
    ) {
        return Err(mismatch());
    }
    Ok(())
}

impl Cfg {
    fn build(
        relation: &PrivateArrayFinalRelationV1<'_>,
        scratch: &mut Scratch,
        work: &mut Work<'_>,
    ) -> Result<Self> {
        let function = relation.function;
        let count = function.blocks().len();
        let entry = function.entry().index() as usize;
        work.charge_private_array_work(3)?;
        if entry >= count
            || relation
                .body
                .blocks
                .first()
                .is_none_or(|block| block.id.0 as usize != entry)
        {
            return Err(mismatch());
        }
        let mut edges = scratch.vector(0, work)?;
        for (from, block) in function.blocks().iter().enumerate() {
            work.charge_private_array_work(1)?;
            block.terminator().kind().try_for_each_edge(|edge| {
                work.charge_private_array_work(2)?;
                let to = edge.target().index() as usize;
                if to >= count {
                    return Err(mismatch());
                }
                scratch.push(&mut edges, [from, to], work)
            })?;
        }
        private_array_heapsort_v1(&mut edges, |row| *row, work, resource)?;
        work.charge_private_array_work(edges.len())?;
        edges.dedup();
        let mut ranges = scratch.vector(count, work)?;
        let mut cursor = 0;
        for from in 0..count {
            work.charge_private_array_work(1)?;
            let start = cursor;
            while edges.get(cursor).is_some_and(|edge| edge[0] == from) {
                work.charge_private_array_work(1)?;
                cursor += 1;
            }
            ranges.push((start, cursor));
        }
        // Ordinary physical paths must remain original source edges. The
        // existing lowering may append its one terminal assertion-failure
        // sink: it has no memory effects or continuation into any source read.
        // Full owner correspondence separately checks its synthetic span.
        let mut physical_blocks = scratch.vector(relation.body.blocks.len(), work)?;
        for block in &relation.body.blocks {
            work.charge_private_array_work(2)?;
            if block.id.0 as usize >= count {
                if block.id.0 as usize != count {
                    return Err(mismatch());
                }
                check_assert_failure_sink(block, work)?;
                physical_blocks.push(block.id.0 as usize);
                continue;
            }
            physical_blocks.push(block.id.0 as usize);
            block
                .terminator
                .as_ref()
                .ok_or_else(mismatch)?
                .try_visit_edges_v1(|to, arguments| {
                    work.charge_private_array_work(4)?;
                    if to.0 as usize == count {
                        if !arguments.is_empty()
                            || !matches!(
                                function.blocks()[block.id.0 as usize].terminator().kind(),
                                SemanticTerminatorKindV1::Assert { .. }
                                    | SemanticTerminatorKindV1::Abort
                                    | SemanticTerminatorKindV1::UnwindTerminate
                            )
                        {
                            return Err(mismatch());
                        }
                        return Ok(());
                    }
                    private_array_binary_search_v1(
                        &edges,
                        |row| *row,
                        [block.id.0 as usize, to.0 as usize],
                        work,
                    )?
                    .map_err(|_| mismatch())?;
                    Ok::<_, Error>(())
                })?;
        }
        private_array_heapsort_v1(&mut physical_blocks, |&block| [block], work, resource)?;
        for pair in physical_blocks.windows(2) {
            work.charge_private_array_work(1)?;
            if pair[0] == pair[1] {
                return Err(mismatch());
            }
        }
        for block in &relation.body.blocks {
            work.charge_private_array_work(1)?;
            block
                .terminator
                .as_ref()
                .ok_or_else(mismatch)?
                .try_visit_edges_v1(|to, _| {
                    private_array_binary_search_v1(
                        &physical_blocks,
                        |&id| [id],
                        [to.0 as usize],
                        work,
                    )?
                    .map_err(|_| mismatch())?;
                    Ok::<_, Error>(())
                })?;
        }
        let physical_bytes = vector_bytes(&physical_blocks)?;
        drop(physical_blocks);
        scratch.release(physical_bytes, work)?;
        let mut reachable = scratch.vector(count, work)?;
        work.charge_private_array_work(count)?;
        reachable.resize(count, false);
        let mut queue = scratch.vector(count, work)?;
        reachable[entry] = true;
        queue.push(entry);
        let mut next = 0;
        while next < queue.len() {
            work.charge_private_array_work(1)?;
            let block = queue[next];
            next += 1;
            let (start, end) = ranges[block];
            for edge in &edges[start..end] {
                work.charge_private_array_work(2)?;
                if !reachable[edge[1]] {
                    reachable[edge[1]] = true;
                    queue.push(edge[1]);
                }
            }
        }
        let queue_bytes = vector_bytes(&queue)?;
        drop(queue);
        scratch.release(queue_bytes, work)?;
        Ok(Self {
            edges,
            ranges,
            reachable,
            entry,
        })
    }
}

fn slot_index(
    relation: &PrivateArrayFinalRelationV1<'_>,
    local: u32,
    work: &mut Work<'_>,
) -> Result<Option<usize>> {
    Ok(private_array_binary_search_v1(
        relation.slots,
        |row| [row.local as usize],
        [local as usize],
        work,
    )?
    .ok())
}

struct Events {
    cells: Vec<CellKey>,
    events: Vec<Event>,
    kills: Vec<Kill>,
    escaped: Vec<u32>,
}
impl Events {
    fn build(
        relation: &PrivateArrayFinalRelationV1<'_>,
        scratch: &mut Scratch,
        work: &mut Work<'_>,
    ) -> Result<Self> {
        let mut this = Self {
            cells: scratch.vector(0, work)?,
            events: scratch.vector(0, work)?,
            kills: scratch.vector(0, work)?,
            escaped: scratch.vector(0, work)?,
        };
        let mut physical = scratch.vector(relation.effects.len(), work)?;
        for (index, effect) in relation.effects.iter().enumerate() {
            work.charge_private_array_work(1)?;
            let slot =
                &relation.slots[slot_index(relation, effect.local, work)?.ok_or_else(mismatch)?];
            let offset = private_array_exact_relation_v1(
                relation.semantic.types(),
                relation.function,
                relation.body,
                relation.owner,
                relation.function_id,
                slot,
                effect,
                relation.max_operations,
                work,
            );
            let offset = match offset {
                Ok(offset) => Some(offset),
                Err(PrivateArrayRelationErrorV1::Work(error)) => return Err(error),
                Err(PrivateArrayRelationErrorV1::Incomplete(_))
                    if effect.access == PrivateArrayAccessV1::Write =>
                {
                    None
                }
                Err(_) => return Err(mismatch()),
            };
            physical.push([
                effect.memory_location.block.0 as usize,
                effect.memory_location.operation,
                effect.semantic_statement as usize,
            ]);
            let Some(offset) = offset else {
                scratch.push(
                    &mut this.kills,
                    Kill {
                        local: effect.local,
                        block: effect.semantic_block as usize,
                        statement: effect.semantic_statement as usize,
                        operation: usize::MAX,
                        phase: 0,
                    },
                    work,
                )?;
                continue;
            };
            usize::try_from(offset).map_err(|_| resource())?;
            let cell = CellKey {
                local: effect.local,
                offset,
            };
            let event = Event {
                cell,
                block: effect.semantic_block as usize,
                statement: effect.semantic_statement as usize,
                operation: effect.memory_location.operation,
                phase: 1,
                action: match effect.access {
                    PrivateArrayAccessV1::Read => Action::Read(index),
                    PrivateArrayAccessV1::Write => Action::Initialize,
                },
            };
            scratch.push(&mut this.events, event, work)?;
            if effect.access == PrivateArrayAccessV1::Read {
                scratch.push(&mut this.cells, cell, work)?;
                if moved_effect(relation, effect, work)? {
                    scratch.push(
                        &mut this.events,
                        Event {
                            phase: 2,
                            action: Action::Kill,
                            ..event
                        },
                        work,
                    )?;
                }
            }
        }
        private_array_heapsort_v1(&mut physical, |row| [row[0], row[1]], work, resource)?;
        for pair in physical.windows(2) {
            work.charge_private_array_work(3)?;
            if pair[0][0] == pair[1][0] && (pair[0][1] == pair[1][1] || pair[0][2] > pair[1][2]) {
                return Err(mismatch());
            }
        }
        let physical_bytes = vector_bytes(&physical)?;
        drop(physical);
        scratch.release(physical_bytes, work)?;
        this.source_kills(relation, scratch, work)?;
        private_array_heapsort_v1(&mut this.cells, |row| row.key(), work, resource)?;
        work.charge_private_array_work(this.cells.len())?;
        this.cells.dedup();
        private_array_heapsort_v1(&mut this.events, |row| row.key(), work, resource)?;
        private_array_heapsort_v1(&mut this.kills, |row| row.key(), work, resource)?;
        private_array_heapsort_v1(&mut this.escaped, |&local| [local as usize], work, resource)?;
        Ok(this)
    }
}

fn moved_effect(
    relation: &PrivateArrayFinalRelationV1<'_>,
    effect: &PrivateArrayEffectV1,
    work: &mut Work<'_>,
) -> Result<bool> {
    use fe2o3_pliron::ProductionSemanticSsaOperandRoleV1 as Role;
    work.charge_private_array_work(1)?;
    let statement = relation.function.blocks()[effect.semantic_block as usize].statements()
        [effect.semantic_statement as usize]
        .kind();
    let operand = match (statement, effect.role) {
        (SemanticStatementKindV1::Assign(assignment), Role::RvalueOperand(ordinal)) => {
            private_array_rvalue_operand_v1(assignment.value().kind(), ordinal, work)?
        }
        (SemanticStatementKindV1::Store(store), Role::StoreValue) => Some(store.value()),
        _ => None,
    };
    Ok(matches!(operand, Some(SemanticOperandV1::Move(_))))
}

impl Events {
    fn kill_local(
        &mut self,
        relation: &PrivateArrayFinalRelationV1<'_>,
        local: u32,
        block: usize,
        statement: usize,
        operation: usize,
        scratch: &mut Scratch,
        work: &mut Work<'_>,
    ) -> Result<()> {
        if slot_index(relation, local, work)?.is_some() {
            scratch.push(
                &mut self.kills,
                Kill {
                    local,
                    block,
                    statement,
                    operation,
                    phase: 0,
                },
                work,
            )?;
        }
        Ok(())
    }
    fn kill_place(
        &mut self,
        relation: &PrivateArrayFinalRelationV1<'_>,
        place: &SemanticPlaceV1,
        block: usize,
        statement: usize,
        operation: usize,
        scratch: &mut Scratch,
        work: &mut Work<'_>,
    ) -> Result<()> {
        let local = place.local().index();
        let Some(slot) = slot_index(relation, local, work)? else {
            return Ok(());
        };
        work.charge_private_array_work(3)?;
        let offset = match place.projections() {
            [projection] => match projection.kind() {
                SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length,
                    from_end,
                } if minimum_length <= relation.slots[slot].length => {
                    if from_end {
                        relation.slots[slot].length.checked_sub(offset)
                    } else {
                        Some(offset)
                    }
                }
                _ => None,
            },
            _ => None,
        }
        .filter(|&offset| offset < relation.slots[slot].length);
        if let Some(offset) = offset {
            scratch.push(
                &mut self.events,
                Event {
                    cell: CellKey { local, offset },
                    block,
                    statement,
                    operation,
                    phase: 0,
                    action: Action::Kill,
                },
                work,
            )?;
        } else {
            scratch.push(
                &mut self.kills,
                Kill {
                    local,
                    block,
                    statement,
                    operation,
                    phase: 0,
                },
                work,
            )?;
        }
        Ok(())
    }
    fn moved(
        &mut self,
        relation: &PrivateArrayFinalRelationV1<'_>,
        operand: &SemanticOperandV1,
        block: usize,
        statement: usize,
        role: Option<fe2o3_pliron::ProductionSemanticSsaOperandRoleV1>,
        scratch: &mut Scratch,
        work: &mut Work<'_>,
    ) -> Result<()> {
        work.charge_private_array_work(1)?;
        let SemanticOperandV1::Move(place) = operand else {
            return Ok(());
        };
        if let Some(role) = role {
            // Recorded projected moves already kill their exact cell after the
            // authenticated physical read. Do not replace this with a whole-slot kill.
            let rows = relation.statement_range(
                SemanticAccessSiteV1 {
                    block: block as u32,
                    statement: Some(statement as u32),
                    ordinal: 0,
                },
                work,
            )?;
            let key = private_array_role_key_v1(role).ok_or_else(mismatch)?;
            let start = private_array_partition_v1(
                rows,
                |row| {
                    let key = private_array_role_key_v1(row.role).unwrap_or((u8::MAX, u32::MAX));
                    [key.0 as usize, key.1 as usize]
                },
                [key.0 as usize, key.1 as usize],
                false,
                work,
            )?;
            let end = private_array_partition_v1(
                rows,
                |row| {
                    let key = private_array_role_key_v1(row.role).unwrap_or((u8::MAX, u32::MAX));
                    [key.0 as usize, key.1 as usize]
                },
                [key.0 as usize, key.1 as usize],
                true,
                work,
            )?;
            for row in &rows[start..end] {
                work.charge_private_array_work(3)?;
                if row.role == role
                    && row.local == place.local().index()
                    && row.access == PrivateArrayAccessV1::Read
                {
                    return Ok(());
                }
            }
        }
        self.kill_place(relation, place, block, statement, usize::MAX, scratch, work)
    }
    fn destination(
        &mut self,
        relation: &PrivateArrayFinalRelationV1<'_>,
        place: &SemanticPlaceV1,
        block: usize,
        statement: usize,
        scratch: &mut Scratch,
        work: &mut Work<'_>,
    ) -> Result<()> {
        if slot_index(relation, place.local().index(), work)?.is_none() {
            return Ok(());
        }
        if place.projections().is_empty() {
            // Every replacement aggregate must establish its own elements.
            return self.kill_local(
                relation,
                place.local().index(),
                block,
                statement,
                0,
                scratch,
                work,
            );
        }
        let rows = relation.statement_range(
            SemanticAccessSiteV1 {
                block: block as u32,
                statement: Some(statement as u32),
                ordinal: 0,
            },
            work,
        )?;
        for row in rows {
            work.charge_private_array_work(2)?;
            if row.local == place.local().index() && row.access == PrivateArrayAccessV1::Write {
                return Ok(());
            }
        }
        self.kill_place(relation, place, block, statement, usize::MAX, scratch, work)
    }
    fn source_kills(
        &mut self,
        relation: &PrivateArrayFinalRelationV1<'_>,
        scratch: &mut Scratch,
        work: &mut Work<'_>,
    ) -> Result<()> {
        use fe2o3_pliron::ProductionSemanticSsaOperandRoleV1 as Role;
        for (block, declaration) in relation.function.blocks().iter().enumerate() {
            work.charge_private_array_work(1)?;
            for (statement, source) in declaration.statements().iter().enumerate() {
                work.charge_private_array_work(1)?;
                match source.kind() {
                    SemanticStatementKindV1::StorageLive(local)
                    | SemanticStatementKindV1::StorageDead(local) => self.kill_local(
                        relation,
                        local.index(),
                        block,
                        statement,
                        0,
                        scratch,
                        work,
                    )?,
                    SemanticStatementKindV1::Deinitialize(place)
                    | SemanticStatementKindV1::SetDiscriminant { place, .. } => {
                        self.kill_place(relation, place, block, statement, 0, scratch, work)?
                    }
                    SemanticStatementKindV1::Assign(assignment) => {
                        self.destination(
                            relation,
                            assignment.destination(),
                            block,
                            statement,
                            scratch,
                            work,
                        )?;
                        if let SemanticRvalueKindV1::Borrow { place, .. }
                        | SemanticRvalueKindV1::AddressOf { place, .. } =
                            assignment.value().kind()
                        {
                            if slot_index(relation, place.local().index(), work)?.is_some() {
                                scratch.push(&mut self.escaped, place.local().index(), work)?;
                            }
                        }
                        private_array_visit_rvalue_operands_v1(
                            assignment.value().kind(),
                            |operand, ordinal| {
                                self.moved(
                                    relation,
                                    operand,
                                    block,
                                    statement,
                                    Some(Role::RvalueOperand(
                                        u32::try_from(ordinal).map_err(|_| resource())?,
                                    )),
                                    scratch,
                                    work,
                                )
                            },
                        )?;
                    }
                    SemanticStatementKindV1::Store(store) => {
                        self.destination(
                            relation,
                            store.destination(),
                            block,
                            statement,
                            scratch,
                            work,
                        )?;
                        self.moved(
                            relation,
                            store.value(),
                            block,
                            statement,
                            Some(Role::StoreValue),
                            scratch,
                            work,
                        )?;
                    }
                    SemanticStatementKindV1::Assume(operand) => {
                        self.moved(relation, operand, block, statement, None, scratch, work)?
                    }
                    SemanticStatementKindV1::AtomicRmw(operation) => {
                        self.kill_place(
                            relation,
                            operation.address(),
                            block,
                            statement,
                            usize::MAX,
                            scratch,
                            work,
                        )?;
                        self.kill_place(
                            relation,
                            operation.destination(),
                            block,
                            statement,
                            usize::MAX,
                            scratch,
                            work,
                        )?;
                        self.moved(
                            relation,
                            operation.value(),
                            block,
                            statement,
                            None,
                            scratch,
                            work,
                        )?;
                    }
                    SemanticStatementKindV1::AtomicCompareExchange(operation) => {
                        self.kill_place(
                            relation,
                            operation.address(),
                            block,
                            statement,
                            usize::MAX,
                            scratch,
                            work,
                        )?;
                        self.kill_place(
                            relation,
                            operation.destination(),
                            block,
                            statement,
                            usize::MAX,
                            scratch,
                            work,
                        )?;
                        self.moved(
                            relation,
                            operation.expected(),
                            block,
                            statement,
                            None,
                            scratch,
                            work,
                        )?;
                        self.moved(
                            relation,
                            operation.replacement(),
                            block,
                            statement,
                            None,
                            scratch,
                            work,
                        )?;
                    }
                    SemanticStatementKindV1::Nop => (),
                }
            }
            let statement = declaration.statements().len();
            work.charge_private_array_work(1)?;
            match declaration.terminator().kind() {
                SemanticTerminatorKindV1::Call(call) => {
                    for operand in call.arguments() {
                        self.moved(relation, operand, block, statement, None, scratch, work)?;
                    }
                    if let Some(destination) = call.destination() {
                        self.kill_place(
                            relation,
                            destination.place(),
                            block,
                            statement,
                            usize::MAX,
                            scratch,
                            work,
                        )?;
                    }
                }
                SemanticTerminatorKindV1::TailCall(call) => {
                    for operand in call.arguments() {
                        self.moved(relation, operand, block, statement, None, scratch, work)?;
                    }
                }
                SemanticTerminatorKindV1::Drop { place, .. } => {
                    self.kill_place(relation, place, block, statement, usize::MAX, scratch, work)?
                }
                SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => self.moved(
                    relation,
                    discriminant,
                    block,
                    statement,
                    None,
                    scratch,
                    work,
                )?,
                SemanticTerminatorKindV1::Assert {
                    condition, message, ..
                } => {
                    self.moved(relation, condition, block, statement, None, scratch, work)?;
                    match message {
                        SemanticAssertMessageV1::BoundsCheck { length, index } => {
                            self.moved(relation, length, block, statement, None, scratch, work)?;
                            self.moved(relation, index, block, statement, None, scratch, work)?;
                        }
                        SemanticAssertMessageV1::Overflow { left, right, .. } => {
                            self.moved(relation, left, block, statement, None, scratch, work)?;
                            self.moved(relation, right, block, statement, None, scratch, work)?;
                        }
                        SemanticAssertMessageV1::DivisionByZero(value)
                        | SemanticAssertMessageV1::RemainderByZero(value) => {
                            self.moved(relation, value, block, statement, None, scratch, work)?
                        }
                        SemanticAssertMessageV1::MisalignedPointerDereference {
                            required_alignment,
                            found_alignment,
                        } => {
                            self.moved(
                                relation,
                                required_alignment,
                                block,
                                statement,
                                None,
                                scratch,
                                work,
                            )?;
                            self.moved(
                                relation,
                                found_alignment,
                                block,
                                statement,
                                None,
                                scratch,
                                work,
                            )?;
                        }
                        SemanticAssertMessageV1::NullPointerDereference
                        | SemanticAssertMessageV1::ResumedAfterReturn
                        | SemanticAssertMessageV1::ResumedAfterPanic => (),
                    }
                }
                SemanticTerminatorKindV1::Goto(_)
                | SemanticTerminatorKindV1::FalseEdge { .. }
                | SemanticTerminatorKindV1::Return
                | SemanticTerminatorKindV1::UnwindResume
                | SemanticTerminatorKindV1::UnwindTerminate
                | SemanticTerminatorKindV1::Abort
                | SemanticTerminatorKindV1::Unreachable => (),
            }
        }
        Ok(())
    }
}

// Identity/initialized/uninitialized are transfer functions, not provenance or
// stored-value equalities. The dual bad-path solver changes each node once.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Transfer {
    Identity,
    Initialized,
    Uninitialized,
}
#[derive(Clone, Copy)]
struct Read {
    effect: usize,
    block: usize,
    requirement: Transfer,
}

fn solve(
    cfg: &Cfg,
    transfer: &[Transfer],
    bad_in: &mut [bool],
    bad_out: &mut [bool],
    queue: &mut Vec<usize>,
    work: &mut Work<'_>,
) -> Result<()> {
    work.charge_private_array_work(bad_in.len().checked_mul(2).ok_or_else(resource)?)?;
    bad_in.fill(false);
    bad_out.fill(false);
    queue.clear();
    bad_in[cfg.entry] = true;
    for block in 0..transfer.len() {
        work.charge_private_array_work(3)?;
        if cfg.reachable[block]
            && (transfer[block] == Transfer::Uninitialized
                || (block == cfg.entry && transfer[block] != Transfer::Initialized))
        {
            bad_out[block] = true;
            queue.push(block);
        }
    }
    let mut next = 0;
    while next < queue.len() {
        work.charge_private_array_work(1)?;
        let block = queue[next];
        next += 1;
        let (start, end) = cfg.ranges[block];
        for edge in &cfg.edges[start..end] {
            work.charge_private_array_work(3)?;
            let target = edge[1];
            bad_in[target] = true;
            if !bad_out[target] && transfer[target] != Transfer::Initialized {
                bad_out[target] = true;
                queue.push(target);
            }
        }
    }
    Ok(())
}

pub(in super::super) fn derive(
    relation: &PrivateArrayFinalRelationV1<'_>,
    work: &mut Work<'_>,
) -> Result<Vec<ReadInitializationV2>> {
    let mut scratch = Scratch::default();
    work.charge_private_array_work(4)?;
    scratch.reserve(
        std::mem::size_of::<(
            Scratch,
            Cfg,
            Events,
            Result<Vec<ReadInitializationV2>>,
            Event,
            Kill,
            Read,
            Option<AmdGpuDiagnosticOperation>,
            (&BasicBlock, &FunctionId, &Vec<ValueId>),
        )>()
        .checked_add(std::mem::size_of::<(usize, usize, usize, usize)>() * 4)
        .ok_or_else(resource)?,
        work,
    )?;
    let cfg = Cfg::build(relation, &mut scratch, work)?;
    let events = Events::build(relation, &mut scratch, work)?;
    let count = relation.function.blocks().len();
    let mut result = scratch.vector(relation.effects.len(), work)?;
    work.charge_private_array_work(relation.effects.len())?;
    result.resize(
        relation.effects.len(),
        ReadInitializationV2 {
            offset: 0,
            initialized: false,
        },
    );
    let mut transfer = scratch.vector(count, work)?;
    let mut bad_in = scratch.vector(count, work)?;
    let mut bad_out = scratch.vector(count, work)?;
    let mut queue = scratch.vector(count, work)?;
    let mut reads = scratch.vector(relation.effects.len(), work)?;
    work.charge_private_array_work(count.checked_mul(3).ok_or_else(resource)?)?;
    transfer.resize(count, Transfer::Identity);
    bad_in.resize(count, false);
    bad_out.resize(count, false);
    for cell in &events.cells {
        if private_array_binary_search_v1(
            &events.escaped,
            |&local| [local as usize],
            [cell.local as usize],
            work,
        )?
        .is_ok()
        {
            continue;
        }
        let begin = private_array_partition_v1(
            &events.events,
            |row| row.cell.key(),
            cell.key(),
            false,
            work,
        )?;
        let end = private_array_partition_v1(
            &events.events,
            |row| row.cell.key(),
            cell.key(),
            true,
            work,
        )?;
        let kill_begin = private_array_partition_v1(
            &events.kills,
            |row| [row.local as usize],
            [cell.local as usize],
            false,
            work,
        )?;
        let kill_end = private_array_partition_v1(
            &events.kills,
            |row| [row.local as usize],
            [cell.local as usize],
            true,
            work,
        )?;
        work.charge_private_array_work(count)?;
        transfer.fill(Transfer::Identity);
        reads.clear();
        let (mut at, mut kill_at) = (begin, kill_begin);
        while at < end || kill_at < kill_end {
            work.charge_private_array_work(5)?;
            if kill_at < kill_end
                && (at == end || events.kills[kill_at].time() <= events.events[at].time())
            {
                transfer[events.kills[kill_at].block] = Transfer::Uninitialized;
                kill_at += 1;
            } else {
                let event = events.events[at];
                at += 1;
                match event.action {
                    Action::Kill => transfer[event.block] = Transfer::Uninitialized,
                    Action::Initialize => transfer[event.block] = Transfer::Initialized,
                    Action::Read(effect) => reads.push(Read {
                        effect,
                        block: event.block,
                        requirement: transfer[event.block],
                    }),
                }
            }
        }
        solve(&cfg, &transfer, &mut bad_in, &mut bad_out, &mut queue, work)?;
        for read in &reads {
            work.charge_private_array_work(3)?;
            result[read.effect] = ReadInitializationV2 {
                offset: cell.offset,
                initialized: cfg.reachable[read.block]
                    && match read.requirement {
                        Transfer::Initialized => true,
                        Transfer::Uninitialized => false,
                        Transfer::Identity => !bad_in[read.block],
                    },
            };
        }
    }
    let retained = vector_bytes(&result)?;
    drop((cfg, events, transfer, bad_in, bad_out, queue, reads));
    scratch.release(
        scratch.bytes.checked_sub(retained).ok_or_else(resource)?,
        work,
    )?;
    Ok(result)
}

#[cfg(test)]
#[path = "production_private_array_cfg_solver_v2_tests.rs"]
mod solver_tests;
