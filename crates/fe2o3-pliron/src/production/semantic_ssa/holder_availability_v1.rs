//! Sparse complete-holder availability, before promotion and SSA planning.

use super::*;
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FieldUpdate {
    pub block: usize,
    pub use_event: usize,
    pub define_event: usize,
    pub local: SsaVariableIdV1,
}

pub(super) trait Meter {
    type Error: From<ProductionSemanticSsaErrorV1>;
    fn work(&mut self, units: usize) -> Result<(), Self::Error>;
    fn reserve(&mut self, bytes: usize) -> Result<(), Self::Error>;
    fn release(&mut self, bytes: usize) -> Result<(), Self::Error>;
}

pub(super) struct PlainMeter;
impl Meter for PlainMeter {
    type Error = ProductionSemanticSsaErrorV1;
    fn work(&mut self, _: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    fn reserve(&mut self, _: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    fn release(&mut self, _: usize) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub(super) struct Account {
    function: SemanticFunctionIdV1,
    limits: ProductionSemanticSsaLimitsV1,
    work: usize,
    storage: usize,
    peak: usize,
    baseline: SemanticSsaAuxiliaryResourcesV1,
    expected_updates: usize,
}

fn overflow() -> ProductionSemanticSsaErrorV1 {
    ProductionSemanticSsaErrorV1::ResourceOverflow
}
fn mismatch() -> ProductionSemanticSsaErrorV1 {
    ProductionSemanticSsaErrorV1::ReplayMismatch
}
fn add(a: usize, b: usize) -> Result<usize, ProductionSemanticSsaErrorV1> {
    a.checked_add(b).ok_or_else(overflow)
}
fn mul(a: usize, b: usize) -> Result<usize, ProductionSemanticSsaErrorV1> {
    a.checked_mul(b).ok_or_else(overflow)
}
fn array<T>(count: usize) -> Result<Vec<T>, ProductionSemanticSsaErrorV1> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| ProductionSemanticSsaErrorV1::HolderAvailabilityAllocation)?;
    if output.capacity() != count {
        return Err(mismatch());
    }
    Ok(output)
}

impl Account {
    pub(super) fn new(
        function: SemanticFunctionIdV1,
        limits: ProductionSemanticSsaLimitsV1,
    ) -> Self {
        Self {
            function,
            limits,
            work: 0,
            storage: 0,
            peak: 0,
            baseline: SemanticSsaAuxiliaryResourcesV1::default(),
            expected_updates: 0,
        }
    }

    pub(super) fn set_baseline(
        &mut self,
        baseline: SemanticSsaAuxiliaryResourcesV1,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        self.baseline = baseline;
        self.check(self.work, self.storage)
    }

    fn check(&self, work: usize, storage: usize) -> Result<(), ProductionSemanticSsaErrorV1> {
        enforce_function_resource_limit_v1(
            self.function,
            SemanticSsaAuxiliaryResourcesV1 {
                work_units: add(self.baseline.work_units, work)?,
                storage_words: add(
                    self.baseline.storage_words,
                    storage.div_ceil(size_of::<usize>()),
                )?,
            },
            self.limits,
        )
    }

    fn count_limit(
        &self,
        resource: SsaPlannerResourceV1,
        required: usize,
        limit: usize,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        if required > limit {
            return Err(ProductionSemanticSsaErrorV1::Planner {
                function: self.function,
                error: SsaPlannerErrorV1::ResourceLimitExceeded {
                    resource,
                    required,
                    limit,
                },
            });
        }
        Ok(())
    }

    fn pay<M: Meter>(
        &mut self,
        work: usize,
        storage: usize,
        meter: &mut M,
    ) -> Result<(), M::Error> {
        let next_work = add(self.work, work)?;
        let next_storage = add(self.storage, storage)?;
        self.check(next_work, next_storage)?;
        meter.work(work)?;
        self.work = next_work;
        meter.reserve(storage)?;
        self.storage = next_storage;
        self.peak = self.peak.max(next_storage);
        Ok(())
    }

    fn release<M: Meter>(&mut self, bytes: usize, meter: &mut M) -> Result<(), M::Error> {
        let next = self.storage.checked_sub(bytes).ok_or_else(mismatch)?;
        meter.release(bytes)?;
        self.storage = next;
        Ok(())
    }

    pub(super) fn markers<M: Meter>(
        &mut self,
        count: usize,
        repeated_projection_work: usize,
        meter: &mut M,
    ) -> Result<Vec<FieldUpdate>, M::Error> {
        let work = add(add(8, repeated_projection_work)?, mul(6, count)?)?;
        let bytes = add(
            add(size_of::<Self>(), size_of::<Vec<FieldUpdate>>())?,
            mul(count, size_of::<FieldUpdate>())?,
        )?;
        self.count_limit(
            SsaPlannerResourceV1::Events,
            mul(2, count)?,
            self.limits.planner().max_events(),
        )?;
        self.pay(work, bytes, meter)?;
        let rows = array(count)?;
        self.expected_updates = count;
        Ok(rows)
    }

    pub(super) fn finish<M: Meter>(
        mut self,
        markers: Vec<FieldUpdate>,
        meter: &mut M,
    ) -> Result<SemanticSsaAuxiliaryResourcesV1, M::Error> {
        drop(markers);
        let resources = SemanticSsaAuxiliaryResourcesV1 {
            storage_words: add(
                self.baseline.storage_words,
                self.peak.div_ceil(size_of::<usize>()),
            )?,
            work_units: add(self.baseline.work_units, self.work)?,
        };
        self.release(self.storage, meter)?;
        Ok(resources)
    }
}

#[derive(Clone, Copy, Debug)]
enum Action {
    Update,
    Present,
    Absent,
}

#[derive(Clone, Copy, Debug)]
struct Row {
    local: SsaVariableIdV1,
    block: usize,
    event: usize,
    action: Action,
}

#[derive(Clone, Copy, Debug)]
struct Summary {
    local: SsaVariableIdV1,
    block: usize,
    end: Option<bool>,
    needs_input: bool,
    bad: bool,
}

impl Summary {
    fn new(local: SsaVariableIdV1, block: usize) -> Self {
        Self {
            local,
            block,
            end: None,
            needs_input: false,
            bad: false,
        }
    }
    fn apply(&mut self, action: Action) {
        match action {
            Action::Present => self.end = Some(true),
            Action::Absent => self.end = Some(false),
            Action::Update => match self.end {
                None => self.needs_input = true,
                Some(false) => self.bad = true,
                Some(true) => {}
            },
        }
    }
}

fn log_bound(n: usize) -> usize {
    usize::BITS as usize - n.leading_zeros() as usize + 1
}

// At most 2*n sifts, each descending at most log_bound(n) levels. Each
// level has two key comparisons and one swap; no library sort or heap scratch.
fn heap_sort<T, K: Ord>(rows: &mut [T], key: impl Fn(&T) -> K) {
    fn sift<T, K: Ord>(rows: &mut [T], mut root: usize, key: &impl Fn(&T) -> K) {
        while root < rows.len() / 2 {
            let left = 2 * root + 1;
            let child = if left + 1 < rows.len() && key(&rows[left]) < key(&rows[left + 1]) {
                left + 1
            } else {
                left
            };
            if key(&rows[root]) >= key(&rows[child]) {
                break;
            }
            rows.swap(root, child);
            root = child;
        }
    }
    for root in (0..rows.len() / 2).rev() {
        sift(rows, root, &key);
    }
    for end in (1..rows.len()).rev() {
        rows.swap(0, end);
        sift(&mut rows[..end], 0, &key);
    }
}

pub(super) fn refine<M: Meter>(
    entry: SsaBlockIdV1,
    promotable: &mut [bool],
    entries: &[SsaVariableIdV1],
    blocks: &[SsaBlockInputV1],
    updates: &[FieldUpdate],
    account: &mut Account,
    meter: &mut M,
) -> Result<(), M::Error> {
    if updates.len() != account.expected_updates {
        return Err(mismatch().into());
    }
    if updates.is_empty() {
        return Ok(());
    }
    if entry.get() as usize >= blocks.len() {
        return Err(mismatch().into());
    }
    account.count_limit(
        SsaPlannerResourceV1::Blocks,
        blocks.len(),
        account.limits.planner().max_blocks(),
    )?;
    account.count_limit(
        SsaPlannerResourceV1::Variables,
        promotable.len(),
        account.limits.planner().max_variables(),
    )?;
    let mut event_count = 0;
    let mut edge_count = 0;
    let mut definition_count = 0;
    for block in blocks {
        account.pay(3, 0, meter)?;
        event_count = add(event_count, block.events().len())?;
        edge_count = add(edge_count, block.edges().len())?;
        if block
            .terminal_failure_start()
            .is_some_and(|start| start > block.events().len())
        {
            return Err(mismatch().into());
        }
        for edge in block.edges() {
            account.pay(2, 0, meter)?;
            definition_count = add(definition_count, edge.definitions().len())?;
            if edge.target().get() as usize >= blocks.len() {
                return Err(mismatch().into());
            }
        }
    }
    account.count_limit(
        SsaPlannerResourceV1::Events,
        event_count,
        account.limits.planner().max_events(),
    )?;
    account.count_limit(
        SsaPlannerResourceV1::Edges,
        edge_count,
        account.limits.planner().max_edges(),
    )?;
    account.count_limit(
        SsaPlannerResourceV1::EdgeDefinitions,
        definition_count,
        account.limits.planner().max_edge_definitions(),
    )?;
    let candidates = updates.len();
    let queue_count = mul(2, blocks.len())?;
    let storage = [
        size_of::<Vec<SsaVariableIdV1>>(),
        mul(candidates, size_of::<SsaVariableIdV1>())?,
        size_of::<Vec<Row>>(),
        mul(event_count, size_of::<Row>())?,
        size_of::<Vec<Summary>>(),
        mul(event_count, size_of::<Summary>())?,
        size_of::<Vec<u8>>(),
        blocks.len(),
        size_of::<Vec<(usize, bool)>>(),
        mul(queue_count, size_of::<(usize, bool)>())?,
    ]
    .into_iter()
    .try_fold(0, add)?;
    let sorting = add(
        mul(mul(16, candidates)?, log_bound(candidates))?,
        mul(mul(16, event_count)?, log_bound(event_count))?,
    )?;
    let indexing = mul(event_count, add(12, log_bound(candidates))?)?;
    let one_walk = add(
        add(
            mul(blocks.len(), add(12, mul(3, log_bound(event_count))?)?)?,
            mul(6, edge_count)?,
        )?,
        add(mul(2, definition_count)?, entries.len())?,
    )?;
    let work = add(
        add(add(16, mul(12, candidates)?)?, sorting)?,
        add(indexing, mul(candidates, one_walk)?)?,
    )?;
    // Both configured limits and capture's actual ledger are checked before
    // allocating any solver array or entering its data-dependent walks.
    account.pay(work, storage, meter)?;
    let result = (|| -> Result<(), ProductionSemanticSsaErrorV1> {
        let mut locals = array(candidates)?;
        let mut previous = None;
        for update in updates {
            let coordinate = (update.block, update.use_event);
            if previous.is_some_and(|old| old >= coordinate)
                || update.define_event != add(update.use_event, 1)?
                || update.local.get() as usize >= promotable.len()
            {
                return Err(mismatch());
            }
            let block = blocks.get(update.block).ok_or_else(mismatch)?;
            if block.events().get(update.use_event) != Some(&SsaEventV1::Use(update.local))
                || block.events().get(update.define_event)
                    != Some(&SsaEventV1::Define(update.local))
                || block
                    .terminal_failure_start()
                    .is_some_and(|start| update.define_event >= start)
            {
                return Err(mismatch());
            }
            previous = Some(coordinate);
            locals.push(update.local);
        }
        heap_sort(&mut locals, |local| local.get());
        locals.dedup();
        let mut rows = array::<Row>(event_count)?;
        let mut marker = 0;
        for (block_id, block) in blocks.iter().enumerate() {
            let normal = block
                .terminal_failure_start()
                .unwrap_or(block.events().len());
            for (event_id, &event) in block.events()[..normal].iter().enumerate() {
                let update = updates.get(marker);
                let action = if update
                    .is_some_and(|row| (row.block, row.use_event) == (block_id, event_id))
                {
                    Some(Action::Update)
                } else if update
                    .is_some_and(|row| (row.block, row.define_event) == (block_id, event_id))
                {
                    marker += 1;
                    None
                } else {
                    match event {
                        SsaEventV1::Define(_) => Some(Action::Present),
                        SsaEventV1::Kill(_) => Some(Action::Absent),
                        SsaEventV1::Use(_) => None,
                    }
                };
                if let Some(action) = action
                    && locals
                        .binary_search_by_key(&event.variable().get(), |local| local.get())
                        .is_ok()
                {
                    rows.push(Row {
                        local: event.variable(),
                        block: block_id,
                        event: event_id,
                        action,
                    });
                }
            }
        }
        if marker != updates.len() {
            return Err(mismatch());
        }
        heap_sort(&mut rows, |row| (row.local.get(), row.block, row.event));
        let mut summaries = array::<Summary>(event_count)?;
        for row in rows {
            if summaries
                .last()
                .is_none_or(|last| (last.local, last.block) != (row.local, row.block))
            {
                summaries.push(Summary::new(row.local, row.block));
            }
            summaries.last_mut().ok_or_else(mismatch)?.apply(row.action);
        }
        let mut visited = array::<u8>(blocks.len())?;
        visited.resize(blocks.len(), 0);
        let mut queue = array::<(usize, bool)>(queue_count)?;
        let mut summary_start = 0;
        for local in locals {
            let summary_end = summary_start
                + summaries[summary_start..].partition_point(|row| row.local == local);
            let local_summaries = &summaries[summary_start..summary_end];
            summary_start = summary_end;
            if !promotable[local.get() as usize] {
                continue;
            }
            visited.fill(0);
            queue.clear();
            let initial = entries.contains(&local);
            visited[entry.get() as usize] = if initial { 1 } else { 2 };
            queue.push((entry.get() as usize, initial));
            let mut next = 0;
            while let Some(&(block_id, present)) = queue.get(next) {
                next += 1;
                let summary = local_summaries
                    .binary_search_by_key(&block_id, |row| row.block)
                    .ok()
                    .map(|index| local_summaries[index]);
                if summary.is_some_and(|row| row.bad || (row.needs_input && !present)) {
                    promotable[local.get() as usize] = false;
                    break;
                }
                let outgoing = summary.and_then(|row| row.end).unwrap_or(present);
                for edge in blocks[block_id].edges() {
                    let state = outgoing || edge.definitions().contains(&local);
                    let bit = if state { 1 } else { 2 };
                    let target = edge.target().get() as usize;
                    if visited[target] & bit == 0 {
                        visited[target] |= bit;
                        queue.push((target, state));
                    }
                }
            }
        }
        Ok(())
    })();
    // The closure owns all five arrays, including error paths. Only the caller's
    // separately prepaid marker vector is still alive at this release.
    account.release(storage, meter)?;
    result.map_err(Into::into)
}

#[cfg(test)]
#[path = "holder_availability_v1_tests.rs"]
mod tests;
