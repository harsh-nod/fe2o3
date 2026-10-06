//! Exact source cut identities and actual scalar micro-cursors. This provides
//! control locations and a zero-step rank, not value, effect or lifetime proof.
//! Candidate coincidence is conservative: a cyclic coincident placement refuses
//! until a separate execution witness establishes nonzero target progress.
use super::super::super::invocations::{CallKind, InvocationPlan};
use super::{Error, Resource, Result, TileTargetV176, Writer, vector};
use fe2o3_lower_mir_kernel::ProductionOptimizedSourceGapV18 as Gap;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdV1 as Block, SemanticCallableDeclV1 as Callable,
    SemanticTerminatorKindV1 as Terminator,
};
use std::{fmt::Write as _, mem::size_of, ops::Range};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Cursor {
    block: usize,
    operation: Option<usize>,
    prefix: usize,
}

#[cfg(test)]
#[path = "original_semantic_mir_tile_microcuts_v180_tests.rs"]
mod tests;

struct Cut {
    instance: usize,
    block: Block,
    candidates: Range<usize>,
    edges: Range<usize>,
}

pub(in super::super) struct TileMicroCutsV180<'target, 'slots, 'view, 'source> {
    target: &'target TileTargetV176<'slots, 'view, 'source>,
    roots: Vec<Range<usize>>,
    cuts: Vec<Cut>,
    candidates: Vec<Cursor>,
    zero_edges: Vec<(usize, usize)>,
    rank: Vec<usize>,
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement("expanded micro-cut differs from its original source or target owner")
}

fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or_else(|| Resource::Arithmetic.into())
}

// Operation None denotes the block's end, after every Some operation. The
// prefix is the monotone key; Option's derived order would put the end first.
fn cursor_key(cursor: Cursor) -> (usize, usize) {
    (cursor.block, cursor.prefix)
}

fn intersects(a: &[Cursor], b: &[Cursor], out: &mut Writer<'_, '_>) -> Result<bool> {
    let (mut left, mut right) = (0, 0);
    while left < a.len() && right < b.len() {
        out.budget.charge_work(2)?;
        match cursor_key(a[left]).cmp(&cursor_key(b[right])) {
            std::cmp::Ordering::Less => left += 1,
            std::cmp::Ordering::Greater => right += 1,
            std::cmp::Ordering::Equal => {
                if a[left] != b[right] {
                    return Err(mismatch());
                }
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn ranks(cuts: &[Cut], edges: &[(usize, usize)], out: &mut Writer<'_, '_>) -> Result<Vec<usize>> {
    let mut degree = vector(cuts.len(), out)?;
    let mut queue = vector(cuts.len(), out)?;
    let mut rank = vector(cuts.len(), out)?;
    out.budget
        .charge_work(cuts.len().checked_mul(2).ok_or(Resource::Arithmetic)?)?;
    degree.resize(cuts.len(), 0usize);
    rank.resize(cuts.len(), 0usize);
    for &(from, to) in edges {
        out.budget.charge_work(2)?;
        if from >= cuts.len() || to >= cuts.len() {
            return Err(mismatch());
        }
        degree[to] = add(degree[to], 1)?;
    }
    for (index, &degree) in degree.iter().enumerate() {
        out.budget.charge_work(1)?;
        if degree == 0 {
            queue.push(index);
        }
    }
    let mut next = 0usize;
    while next < queue.len() {
        out.budget.charge_work(3)?;
        let index = queue[next];
        rank[index] = cuts.len().checked_sub(next).ok_or(Resource::Arithmetic)?;
        next += 1;
        for &(from, to) in edges.get(cuts[index].edges.clone()).ok_or_else(mismatch)? {
            out.budget.charge_work(3)?;
            if from != index || degree[to] == 0 {
                return Err(mismatch());
            }
            degree[to] -= 1;
            if degree[to] == 0 {
                if queue.len() == queue.capacity() {
                    return Err(Resource::Accounting.into());
                }
                queue.push(to);
            }
        }
    }
    if next != cuts.len() {
        return Err(Error::Statement(
            "expanded source cuts contain a possible zero-target-step cycle",
        ));
    }
    let credit = add(degree.capacity(), queue.capacity())?
        .checked_mul(size_of::<usize>())
        .ok_or(Resource::Arithmetic)?;
    drop(degree);
    drop(queue);
    out.budget.release_storage(credit)?;
    Ok(rank)
}

impl<'target, 'slots, 'view, 'source> TileMicroCutsV180<'target, 'slots, 'view, 'source> {
    pub(in super::super) fn derive(
        target: &'target TileTargetV176<'slots, 'view, 'source>,
        plan: &InvocationPlan<'_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        target.slots.with_source_query_v42(out, |out| {
            target.check(out)?;
            out.budget.reserve_storage(
                size_of::<Self>()
                    + 2 * size_of::<Result<Self>>()
                    + 8 * (size_of::<Vec<usize>>() + 2 * size_of::<Result<Vec<usize>>>())
                    + 32 * size_of::<usize>(),
            )?;
            let source = target.slots.correspondence(out)?.source(out.budget)?;
            if !std::ptr::eq(source, plan.source(out)?) {
                return Err(mismatch());
            }
            let semantic = source.source_semantic(out.budget)?;
            let tile = target.slots.tile_owner_v176(out)?;
            let root_count = source.root_count(out.budget)?;
            if root_count != target.roots.len() {
                return Err(mismatch());
            }
            let (mut count, mut candidate_count, mut edge_capacity) = (0usize, 0usize, 0usize);
            for root in 0..root_count {
                for instance in 0..plan.root(root, out)?.instances.len() {
                    let row = plan.instance(root, instance, out)?;
                    let function = semantic
                        .functions()
                        .get(row.function.index() as usize)
                        .ok_or_else(mismatch)?;
                    if row.blocks.start != count || row.blocks.len() != function.blocks().len() {
                        return Err(mismatch());
                    }
                    count = add(count, row.blocks.len())?;
                    for (ordinal, block) in function.blocks().iter().enumerate() {
                        out.budget.charge_work(1)?;
                        block.terminator().kind().try_for_each_edge(|_| {
                            out.budget.charge_work(1)?;
                            edge_capacity = add(edge_capacity, 1)?;
                            Ok::<_, Error>(())
                        })?;
                        let start = out.budget.storage();
                        let block = Block::from_index(
                            u32::try_from(ordinal).map_err(|_| Resource::Arithmetic)?,
                        );
                        let gap =
                            tile.source_block_entry_gap_v177(root, instance, block, out.budget)?;
                        if let Some(gap) = &gap {
                            if !row.active {
                                return Err(mismatch());
                            }
                            if matches!(gap.prefix_disposition(out.budget)?, Gap::Reachable(_)) {
                                candidate_count =
                                    add(candidate_count, gap.candidate_count(out.budget)?)?;
                            }
                        }
                        drop(gap);
                        out.budget.release_storage(
                            out.budget
                                .storage()
                                .checked_sub(start)
                                .ok_or(Resource::Accounting)?,
                        )?;
                    }
                }
            }
            // At most one invocation entry and one return continuation per cut.
            edge_capacity = add(
                edge_capacity,
                count.checked_mul(2).ok_or(Resource::Arithmetic)?,
            )?;
            let mut result = Self {
                target,
                roots: vector(root_count, out)?,
                cuts: vector(count, out)?,
                candidates: vector(candidate_count, out)?,
                zero_edges: vector(edge_capacity, out)?,
                rank: vector(0, out)?,
                required: 0,
            };
            for root in 0..root_count {
                let first = result.cuts.len();
                let function = target.root_function(root, out)?;
                for instance in 0..plan.root(root, out)?.instances.len() {
                    let row = plan.instance(root, instance, out)?;
                    for ordinal in 0..row.blocks.len() {
                        out.budget.charge_work(2)?;
                        let block = Block::from_index(
                            u32::try_from(ordinal).map_err(|_| Resource::Arithmetic)?,
                        );
                        let first_candidate = result.candidates.len();
                        let start = out.budget.storage();
                        let gap =
                            tile.source_block_entry_gap_v177(root, instance, block, out.budget)?;
                        if let Some(gap) = &gap
                            && matches!(gap.prefix_disposition(out.budget)?, Gap::Reachable(_))
                        {
                            for candidate in 0..gap.candidate_count(out.budget)? {
                                let point = gap.candidate(candidate, out.budget)?;
                                if point.first != point.last || point.block.function != function {
                                    return Err(mismatch());
                                }
                                let inventory = target.inventory(out)?;
                                out.budget.charge_work(
                                    (usize::BITS - inventory.blocks().len().leading_zeros())
                                        as usize
                                        + 5,
                                )?;
                                let block = inventory
                                    .blocks()
                                    .binary_search_by_key(&point.block, |row| row.coordinate)
                                    .map_err(|_| mismatch())?;
                                let physical = &inventory.blocks()[block];
                                let prefix = point.first as usize;
                                if prefix > physical.operations.len() {
                                    return Err(mismatch());
                                }
                                let operation = if prefix == physical.operations.len() {
                                    None
                                } else {
                                    Some(add(physical.operations.start, prefix)?)
                                };
                                let cursor = Cursor {
                                    block,
                                    operation,
                                    prefix,
                                };
                                if result.candidates.len() == result.candidates.capacity()
                                    || (result.candidates.len() > first_candidate
                                        && cursor_key(
                                            *result.candidates.last().ok_or_else(mismatch)?,
                                        ) > cursor_key(cursor))
                                {
                                    return Err(mismatch());
                                }
                                result.candidates.push(cursor);
                            }
                        }
                        drop(gap);
                        out.budget.release_storage(
                            out.budget
                                .storage()
                                .checked_sub(start)
                                .ok_or(Resource::Accounting)?,
                        )?;
                        result.cuts.push(Cut {
                            instance,
                            block,
                            candidates: first_candidate..result.candidates.len(),
                            edges: 0..0,
                        });
                    }
                }
                result.roots.push(first..result.cuts.len());
            }
            if result.cuts.len() != count || result.candidates.len() != candidate_count {
                return Err(mismatch());
            }
            result.derive_edges(plan, out)?;
            result.rank = ranks(&result.cuts, &result.zero_edges, out)?;
            result.required = out.budget.storage();
            result.check(out)?;
            Ok(result)
        })
    }

    fn zero_edge(
        &mut self,
        from: usize,
        to: usize,
        root: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(2)?;
        let range = self.roots.get(root).ok_or_else(mismatch)?;
        if !range.contains(&from) || !range.contains(&to) {
            return Err(mismatch());
        }
        let a = &self.candidates[self.cuts[from].candidates.clone()];
        let b = &self.candidates[self.cuts[to].candidates.clone()];
        if intersects(a, b, out)? {
            if self.zero_edges.len() == self.zero_edges.capacity() {
                return Err(Resource::Accounting.into());
            }
            self.zero_edges.push((from, to));
        }
        Ok(())
    }

    fn derive_edges(
        &mut self,
        plan: &InvocationPlan<'_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let source = plan.source(out)?;
        let semantic = source.source_semantic(out.budget)?;
        for root in 0..self.roots.len() {
            for pc in self.roots[root].clone() {
                out.budget.charge_work(2)?;
                let first = self.zero_edges.len();
                let instance = self.cuts[pc].instance;
                let block = self.cuts[pc].block;
                let row = plan.instance(root, instance, out)?;
                let function = semantic
                    .functions()
                    .get(row.function.index() as usize)
                    .ok_or_else(mismatch)?;
                let terminator = function
                    .blocks()
                    .get(block.index() as usize)
                    .ok_or_else(mismatch)?
                    .terminator()
                    .kind();
                terminator.try_for_each_edge(|edge| {
                    out.budget.charge_work(1)?;
                    self.zero_edge(
                        pc,
                        add(row.blocks.start, edge.target().index() as usize)?,
                        root,
                        out,
                    )
                })?;
                let calls = plan.calls(root, instance, out)?;
                out.budget
                    .charge_work((usize::BITS - calls.len().leading_zeros()) as usize + 1)?;
                if let Ok(at) =
                    calls.binary_search_by_key(&block.index(), |call| call.block.index())
                {
                    let call = &calls[at];
                    if let Some(child) = call.child {
                        if call.kind != CallKind::Direct {
                            return Err(Error::Statement(
                                "expanded micro-cuts require direct invocation continuations",
                            ));
                        }
                        let child = plan.instance(root, child, out)?;
                        let declaration = semantic
                            .functions()
                            .get(child.function.index() as usize)
                            .ok_or_else(mismatch)?;
                        self.zero_edge(
                            pc,
                            add(child.blocks.start, declaration.entry().index() as usize)?,
                            root,
                            out,
                        )?;
                    } else if matches!(
                        semantic.callables().get(call.callable.index() as usize),
                        Some(Callable::Defined { .. })
                    ) && !self.cuts[pc].candidates.is_empty()
                    {
                        return Err(Error::Statement(
                            "expanded micro-cut invocation has no authenticated child",
                        ));
                    }
                }
                if matches!(terminator, Terminator::Return)
                    && let Some((parent, call_block)) = row.incoming
                {
                    let parent = plan.instance(root, parent, out)?;
                    let declaration = semantic
                        .functions()
                        .get(parent.function.index() as usize)
                        .ok_or_else(mismatch)?;
                    let Terminator::Call(call) = declaration
                        .blocks()
                        .get(call_block.index() as usize)
                        .ok_or_else(mismatch)?
                        .terminator()
                        .kind()
                    else {
                        return Err(Error::Statement(
                            "expanded micro-cut return continuation is unsupported",
                        ));
                    };
                    let destination = call.destination().ok_or_else(mismatch)?;
                    self.zero_edge(
                        pc,
                        add(
                            parent.blocks.start,
                            destination.edge().target().index() as usize,
                        )?,
                        root,
                        out,
                    )?;
                }
                self.cuts[pc].edges = first..self.zero_edges.len();
            }
        }
        Ok(())
    }

    fn check(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.target.check(out)?;
        self.target
            .slots
            .check_query_storage_floor(self.required, out.budget)
    }

    pub(in super::super) fn emit(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check(out)?;
        for (root, range) in self.roots.iter().enumerate() {
            write!(out, "spec fn invocation_tile_cursor_{root}_v180(source_pc: int, m: MemoryMicroStateV30) -> bool {{ m.state.valid && (false")
                .map_err(|_| out.error())?;
            for pc in range.clone() {
                out.budget.charge_work(1)?;
                write!(out, " || (source_pc == {pc} && (false").map_err(|_| out.error())?;
                for cursor in &self.candidates[self.cuts[pc].candidates.clone()] {
                    out.budget.charge_work(1)?;
                    let operation = cursor.operation.map(|value| value as i128).unwrap_or(-1);
                    write!(out, " || (m.state.pc == {} && m.next_operation == {operation} && m.observations.len() == {})", cursor.block, cursor.prefix)
                        .map_err(|_| out.error())?;
                }
                write!(out, "))").map_err(|_| out.error())?;
            }
            write!(
                out,
                ") }}\nspec fn invocation_tile_zero_rank_{root}_v180(pc: int) -> nat {{\n"
            )
            .map_err(|_| out.error())?;
            for pc in range.clone() {
                out.budget.charge_work(1)?;
                write!(out, " if pc == {pc} {{ {}nat }} else", self.rank[pc])
                    .map_err(|_| out.error())?;
            }
            write!(out, " {{ 0nat }}\n}}\nspec fn invocation_tile_zero_edge_{root}_v180(from: int, to: int) -> bool {{ false").map_err(|_| out.error())?;
            for pc in range.clone() {
                for &(from, to) in &self.zero_edges[self.cuts[pc].edges.clone()] {
                    out.budget.charge_work(1)?;
                    write!(out, " || (from == {from} && to == {to})").map_err(|_| out.error())?;
                }
            }
            write!(out, " }}\nproof fn invocation_tile_zero_edge_decreases_{root}_v180(from: int, to: int)\n requires invocation_tile_zero_edge_{root}_v180(from, to),\n ensures invocation_tile_zero_rank_{root}_v180(from) > invocation_tile_zero_rank_{root}_v180(to),\n{{ }}\n")
                .map_err(|_| out.error())?;
        }
        self.check(out)
    }
}
