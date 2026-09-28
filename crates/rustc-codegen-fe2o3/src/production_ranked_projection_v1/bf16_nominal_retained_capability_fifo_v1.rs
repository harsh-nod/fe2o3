//! Inert retained original-order FIFO pass. No source/facts/capability authority.
//! The enclosing driver must supply the true source transfer and retain this
//! entire owner outside every checked-query postflight.
use super::super::{
    MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1, checked_capability_stored_entries_v1,
};
use super::*;
type Identity = (usize, CanonicalKernelIrWorkLedgerIdentityV1);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Fresh,
    Terminal,
    Complete,
}

pub(super) struct RetainedCapabilityFifoPassV1 {
    phase: Phase,
    ledger: Option<Identity>,
    function: Option<usize>,
    entries: Vec<Slot>,
    reached: Vec<bool>,
    scratch: Vec<Slot>,
    queue: Vec<usize>,
    head: usize,
    successors: Vec<usize>,
    visits: Vec<usize>,
    stored: usize,
    current: Option<usize>,
}
pub(super) struct CompletedFifoPassV1<'a> {
    pub(super) entries: &'a [Slot],
    pub(super) reached: &'a [bool],
    pub(super) visits: &'a [usize],
}
impl RetainedCapabilityFifoPassV1 {
    pub(super) fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            ledger: None,
            function: None,
            entries: Vec::new(),
            reached: Vec::new(),
            scratch: Vec::new(),
            queue: Vec::new(),
            head: 0,
            successors: Vec::new(),
            visits: Vec::new(),
            stored: 0,
            current: None,
        }
    }
    pub(super) fn prepare_into<F>(
        &mut self,
        function: &SemanticFunctionDeclV1,
        consumer: &mut dyn NominalCapabilityConsumerV1,
        owned: &mut usize,
        original_work: &mut usize,
        mut transfer: F,
    ) -> Result<()>
    where
        F: FnMut(usize, &mut [Slot], &mut dyn NominalCapabilityConsumerV1) -> Result<()>,
    {
        let fresh = self.phase == Phase::Fresh && self.ledger.is_none() && self.function.is_none();
        self.phase = Phase::Terminal;
        let ledger = consumer.ledger_v1();
        if !fresh || ledger.denied_work || ledger.denied_storage {
            return Err(Error::Resource(Resource::Accounting));
        }
        consumer.charge_work_v1(32)?;
        retain(consumer, owned, fifo_frame::<F>()?)?;
        self.ledger = Some((ledger.slot, ledger.identity));
        self.function = Some(function as *const SemanticFunctionDeclV1 as usize);
        let blocks = function.blocks().len();
        let locals = function.locals().len();
        require(
            (1..=MAX_BLOCKS).contains(&blocks) && (1..=MAX_LOCALS).contains(&locals),
            "retained capability FIFO exceeds its bounded block/local shape",
        )?;
        let entry = function.entry().index() as usize;
        if entry >= blocks {
            return Err(transfer_error(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "a capability projection entry outside the semantic CFG",
                ),
            ));
        }
        fill(
            &mut self.entries,
            mul(blocks, locals)?,
            None,
            consumer,
            owned,
        )?;
        fill(&mut self.reached, blocks, false, consumer, owned)?;
        fill(&mut self.scratch, locals, None, consumer, owned)?;
        self.reached[entry] = true;
        push(&mut self.queue, entry, consumer, owned)?;
        while self.head < self.queue.len() {
            consumer.charge_work_v1(1)?;
            let block_index = self.queue[self.head];
            self.head = add(self.head, 1)?;
            self.current = Some(block_index);
            push(&mut self.visits, block_index, consumer, owned)?;
            let start = mul(block_index, locals)?;
            if !self.reached.get(block_index).copied().unwrap_or(false) {
                return Err(transfer_error(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "a queued capability dataflow block has no entry state",
                    ),
                ));
            }
            let count = populated_budget(&self.entries[start..start + locals], consumer)?;
            original_charge(consumer, original_work, add(count, 1)?)?;
            checked_capability_stored_entries_v1(0, count).map_err(transfer_error)?;
            consumer.charge_work_v1(locals)?;
            self.scratch
                .copy_from_slice(&self.entries[start..start + locals]);
            transfer(block_index, &mut self.scratch, consumer)?;
            let count = populated_budget(&self.scratch, consumer)?;
            original_charge(
                consumer,
                original_work,
                function.blocks()[block_index]
                    .statements()
                    .len()
                    .checked_add(count)
                    .ok_or_else(|| {
                        transfer_error(ProductionRankedProjectionErrorV1::Unsupported(
                            "capability dataflow work overflow",
                        ))
                    })?,
            )?;
            if count > MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1 {
                return Err(transfer_error(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "capability dataflow exceeds the charged projection limit",
                    ),
                ));
            }
            collect_successors(
                function.blocks()[block_index].terminator().kind(),
                count,
                &mut self.successors,
                consumer,
                owned,
                original_work,
            )?;
            for index in 0..self.successors.len() {
                consumer.charge_work_v1(1)?;
                let target = self.successors[index];
                if target >= blocks {
                    return Err(transfer_error(
                        ProductionRankedProjectionErrorV1::Unsupported(
                            "a capability CFG edge outside the semantic function",
                        ),
                    ));
                }
                let start = mul(target, locals)?;
                let row = &mut self.entries[start..start + locals];
                let changed = if !self.reached[target] {
                    let next_stored = checked_capability_stored_entries_v1(self.stored, count)
                        .map_err(transfer_error)?;
                    checked_capability_stored_entries_v1(0, count).map_err(transfer_error)?;
                    consumer.charge_work_v1(locals)?;
                    row.copy_from_slice(&self.scratch);
                    self.stored = next_stored;
                    self.reached[target] = true;
                    true
                } else {
                    merge_original(row, &self.scratch, &mut self.stored, consumer)?
                };
                if changed {
                    push(&mut self.queue, target, consumer, owned)?;
                }
            }
        }
        let after = consumer.ledger_v1();
        if after.denied_work
            || after.denied_storage
            || self.ledger != Some((after.slot, after.identity))
        {
            return Err(Error::Resource(Resource::Accounting));
        }
        self.phase = Phase::Complete;
        Ok(())
    }
    pub(super) fn completed_for<'a>(
        &'a self,
        function: &SemanticFunctionDeclV1,
        consumer: &dyn NominalCapabilityConsumerV1,
    ) -> Result<CompletedFifoPassV1<'a>> {
        let ledger = consumer.ledger_v1();
        if self.phase != Phase::Complete
            || ledger.denied_work
            || ledger.denied_storage
            || self.ledger != Some((ledger.slot, ledger.identity))
            || self.function != Some(function as *const SemanticFunctionDeclV1 as usize)
        {
            return Err(Error::Resource(Resource::Accounting));
        }
        Ok(CompletedFifoPassV1 {
            entries: &self.entries,
            reached: &self.reached,
            visits: &self.visits,
        })
    }
}

// Actual accepted storage belongs to the enclosing counter/owner, never refunded here.
fn retain(
    consumer: &mut dyn NominalCapabilityConsumerV1,
    owned: &mut usize,
    bytes: usize,
) -> Result<()> {
    let next = add(*owned, bytes)?;
    consumer.reserve_storage_v1(bytes)?;
    *owned = next;
    Ok(())
}
fn reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    consumer: &mut dyn NominalCapabilityConsumerV1,
    owned: &mut usize,
) -> Result<()> {
    let requested = add(values.len(), additional)?;
    if requested <= values.capacity() {
        return Ok(());
    }
    // Same conservative retained-growth policy as original preparation: full
    // next capacity is charged while all prior accepted growth stays owned.
    consumer.charge_work_v1(values.len())?;
    retain(consumer, owned, mul(requested, size_of::<T>())?)?;
    values
        .try_reserve_exact(additional)
        .map_err(|_| Error::Resource(Resource::Allocation))?;
    if size_of::<T>() != 0 && values.capacity() != requested {
        return Err(Error::Resource(Resource::Allocation));
    }
    Ok(())
}
fn fill<T: Copy>(
    values: &mut Vec<T>,
    count: usize,
    value: T,
    consumer: &mut dyn NominalCapabilityConsumerV1,
    owned: &mut usize,
) -> Result<()> {
    consumer.charge_work_v1(count)?;
    reserve(values, count, consumer, owned)?;
    values.resize(count, value);
    Ok(())
}
fn push(
    values: &mut Vec<usize>,
    value: usize,
    consumer: &mut dyn NominalCapabilityConsumerV1,
    owned: &mut usize,
) -> Result<()> {
    consumer.charge_work_v1(1)?;
    reserve(values, 1, consumer, owned)?;
    values.push(value);
    Ok(())
}
fn original_charge(
    consumer: &mut dyn NominalCapabilityConsumerV1,
    original_work: &mut usize,
    amount: usize,
) -> Result<()> {
    consumer.charge_work_v1(amount)?;
    charge_capability_dataflow_work_v1(original_work, amount).map_err(transfer_error)
}
fn populated_budget(
    slots: &[Slot],
    consumer: &mut dyn NominalCapabilityConsumerV1,
) -> Result<usize> {
    consumer.charge_work_v1(slots.len())?;
    let mut count = 0usize;
    for slot in slots {
        if slot.is_some() {
            count = add(count, 1)?;
        }
    }
    Ok(count)
}
fn collect_successors(
    terminator: &SemanticTerminatorKindV1,
    state_count: usize,
    successors: &mut Vec<usize>,
    consumer: &mut dyn NominalCapabilityConsumerV1,
    owned: &mut usize,
    original_work: &mut usize,
) -> Result<()> {
    consumer.charge_work_v1(successors.len())?;
    successors.clear();
    terminator.try_for_each_edge::<Error>(|edge| {
        original_charge(consumer, original_work, 1)?;
        let target = edge.target().index() as usize;
        consumer.charge_work_v1(successors.len())?;
        if !successors.contains(&target) {
            push(successors, target, consumer, owned)?;
        }
        Ok(())
    })?;
    // Bounded explicit insertion sort of unique targets, not raw edge order.
    for end in 1..successors.len() {
        consumer.charge_work_v1(1)?;
        let value = successors[end];
        let mut cursor = end;
        while cursor > 0 {
            consumer.charge_work_v1(1)?;
            if successors[cursor - 1] <= value {
                break;
            }
            consumer.charge_work_v1(1)?;
            successors[cursor] = successors[cursor - 1];
            cursor -= 1;
        }
        consumer.charge_work_v1(1)?;
        successors[cursor] = value;
    }
    let merge_work = state_count.checked_add(1).ok_or_else(|| {
        transfer_error(ProductionRankedProjectionErrorV1::Unsupported(
            "capability dataflow work overflow",
        ))
    })?;
    // Complete this loop before the caller mutates ANY successor row.
    for _ in successors.iter() {
        original_charge(consumer, original_work, merge_work)?;
    }
    Ok(())
}
fn merge_original(
    current: &mut [Slot],
    incoming: &[Slot],
    stored: &mut usize,
    consumer: &mut dyn NominalCapabilityConsumerV1,
) -> Result<bool> {
    require(
        current.len() == incoming.len(),
        "retained FIFO merge row lengths differ",
    )?;
    consumer.charge_work_v1(mul(current.len(), 8)?)?;
    let mut current_count = 0usize;
    let mut additional = 0usize;
    for (old, next) in current.iter().zip(incoming.iter()) {
        current_count = add(current_count, usize::from(old.is_some()))?;
        additional = add(additional, usize::from(old.is_none() && next.is_some()))?;
    }
    let next_stored =
        checked_capability_stored_entries_v1(*stored, additional).map_err(transfer_error)?;
    checked_capability_stored_entries_v1(current_count, additional).map_err(transfer_error)?;
    let mut changed = false;
    for (old, next) in current.iter_mut().zip(incoming.iter().copied()) {
        let merged = match (*old, next) {
            (Some(a), Some(b)) => Some(merge_capability_values_v1(a, b)),
            (Some(_), None) | (None, Some(_)) => Some(ProjectedCapabilityValueV1::Invalid),
            (None, None) => None,
        };
        changed |= *old != merged;
        *old = merged;
    }
    *stored = next_stored;
    Ok(changed)
}

const FIFO_FRAME_ROWS: usize = 31;
fn reserve_frame<T>() -> usize {
    size_of::<(
        &mut Vec<T>,
        usize,
        &mut dyn NominalCapabilityConsumerV1,
        &mut usize,
        usize,
        usize,
        Result<()>,
        std::collections::TryReserveError,
        std::result::Result<(), std::collections::TryReserveError>,
        Error,
        Resource,
    )>()
}
fn fill_frame<T>() -> usize {
    size_of::<(
        &mut Vec<T>,
        usize,
        T,
        &mut dyn NominalCapabilityConsumerV1,
        &mut usize,
        Result<()>,
    )>()
}
fn fifo_frame_rows<F>() -> Result<[usize; FIFO_FRAME_ROWS]> {
    Ok([
        size_of::<RetainedCapabilityFifoPassV1>(),
        size_of::<(
            Phase,
            Option<Identity>,
            Option<usize>,
            Vec<Slot>,
            Vec<bool>,
            Vec<Slot>,
            Vec<usize>,
            Vec<usize>,
            Vec<usize>,
            usize,
            usize,
            Option<usize>,
        )>(),
        size_of::<(
            &mut RetainedCapabilityFifoPassV1,
            &SemanticFunctionDeclV1,
            &mut dyn NominalCapabilityConsumerV1,
            &mut usize,
            &mut usize,
            F,
            Result<()>,
        )>(),
        size_of::<(
            NominalCapabilityLedgerV1,
            NominalCapabilityLedgerV1,
            Identity,
            Option<Identity>,
            Option<usize>,
            bool,
        )>(),
        size_of::<(
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            bool,
            bool,
            Option<&bool>,
            Option<bool>,
            std::ops::Range<usize>,
            std::ops::RangeInclusive<usize>,
            std::ops::RangeInclusive<usize>,
        )>(),
        size_of::<(
            &mut F,
            usize,
            &mut Vec<Slot>,
            &mut [Slot],
            &mut dyn NominalCapabilityConsumerV1,
            Result<()>,
        )>(),
        size_of::<(
            &Vec<Slot>,
            &[Slot],
            &mut Vec<Slot>,
            &mut [Slot],
            usize,
            Result<usize>,
            Result<()>,
        )>(),
        size_of::<(
            &SemanticTerminatorKindV1,
            usize,
            &mut Vec<usize>,
            &mut dyn NominalCapabilityConsumerV1,
            &mut usize,
            &mut usize,
            Result<()>,
        )>(),
        size_of::<(
            &mut Vec<Slot>,
            &mut [Slot],
            &Vec<Slot>,
            &[Slot],
            &mut usize,
            &mut dyn NominalCapabilityConsumerV1,
            Result<bool>,
            bool,
        )>(),
        size_of::<(
            &mut Vec<usize>,
            usize,
            &mut dyn NominalCapabilityConsumerV1,
            &mut usize,
            Result<()>,
        )>(),
        size_of::<(
            &mut Vec<usize>,
            usize,
            &mut dyn NominalCapabilityConsumerV1,
            &mut usize,
            Result<()>,
        )>(),
        size_of::<(
            &RetainedCapabilityFifoPassV1,
            &SemanticFunctionDeclV1,
            &dyn NominalCapabilityConsumerV1,
            NominalCapabilityLedgerV1,
            Option<Identity>,
            Option<usize>,
            bool,
            CompletedFifoPassV1<'static>,
            Result<CompletedFifoPassV1<'static>>,
        )>(),
        size_of::<(
            &Vec<Slot>,
            &[Slot],
            &Vec<bool>,
            &[bool],
            &Vec<usize>,
            &[usize],
        )>(),
        size_of::<(
            &mut dyn NominalCapabilityConsumerV1,
            &mut usize,
            usize,
            usize,
            Result<()>,
        )>(),
        reserve_frame::<Slot>(),
        reserve_frame::<bool>(),
        reserve_frame::<usize>(),
        fill_frame::<Slot>(),
        fill_frame::<bool>(),
        size_of::<(
            &mut Vec<usize>,
            usize,
            &mut dyn NominalCapabilityConsumerV1,
            &mut usize,
            Result<()>,
        )>(),
        size_of::<(
            &mut dyn NominalCapabilityConsumerV1,
            &mut usize,
            usize,
            std::result::Result<(), ProductionRankedProjectionErrorV1>,
            ProductionRankedProjectionErrorV1,
            Result<()>,
        )>(),
        size_of::<(
            &[Slot],
            &mut dyn NominalCapabilityConsumerV1,
            std::slice::Iter<'static, Slot>,
            &Slot,
            usize,
            Result<usize>,
            Option<usize>,
            bool,
        )>(),
        size_of::<(
            &SemanticTerminatorKindV1,
            usize,
            &mut Vec<usize>,
            &mut dyn NominalCapabilityConsumerV1,
            &mut usize,
            &mut usize,
            Result<()>,
        )>(),
        size_of::<(
            (
                &mut Vec<usize>,
                &mut dyn NominalCapabilityConsumerV1,
                &mut usize,
                &mut usize,
            ),
            fe2o3_mir_model::semantic_mir_v1::SemanticControlFlowEdgeV1,
            fe2o3_mir_model::semantic_mir_v1::SemanticControlFlowEdgeV1,
            fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1,
            fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1,
            u32,
            usize,
            bool,
            Result<()>,
        )>(),
        size_of::<(
            std::ops::Range<usize>,
            usize,
            usize,
            usize,
            usize,
            &usize,
            std::slice::Iter<'static, usize>,
            Option<usize>,
            Result<()>,
        )>(),
        size_of::<(
            &mut [Slot],
            &[Slot],
            &mut usize,
            &mut dyn NominalCapabilityConsumerV1,
            usize,
            usize,
            usize,
            bool,
            Result<bool>,
        )>(),
        size_of::<(
            std::iter::Zip<std::slice::Iter<'static, Slot>, std::slice::Iter<'static, Slot>>,
            (&Slot, &Slot),
            &Slot,
            &Slot,
            bool,
            usize,
            Result<usize>,
        )>(),
        size_of::<(
            std::iter::Zip<
                std::slice::IterMut<'static, Slot>,
                std::iter::Copied<std::slice::Iter<'static, Slot>>,
            >,
            (&mut Slot, Slot),
            &mut Slot,
            Slot,
            Slot,
            ProjectedCapabilityValueV1,
            ProjectedCapabilityValueV1,
            bool,
        )>(),
        size_of::<(
            usize,
            usize,
            usize,
            Result<usize>,
            std::result::Result<(), ProductionRankedProjectionErrorV1>,
            std::result::Result<usize, ProductionRankedProjectionErrorV1>,
            ProductionRankedProjectionErrorV1,
            Error,
            Resource,
            Result<()>,
        )>(),
        size_of::<(
            [usize; FIFO_FRAME_ROWS],
            Result<[usize; FIFO_FRAME_ROWS]>,
            std::array::IntoIter<usize, FIFO_FRAME_ROWS>,
            usize,
            usize,
            Option<usize>,
            Result<usize>,
        )>(),
        size_of::<(
            F,
            &mut F,
            Error,
            Resource,
            Result<usize>,
            bool,
            Option<usize>,
            Option<Identity>,
        )>(),
    ])
}
fn fifo_frame<F>() -> Result<usize> {
    fifo_frame_rows::<F>()?
        .into_iter()
        .try_fold(0usize, |sum, row| add(sum, row))
}
#[cfg(test)]
#[path = "bf16_nominal_retained_capability_fifo_v1_tests.rs"]
mod tests;
