//! Batch-owned source inventory only. Every selected use still needs the live
//! source, SSA, loan and epoch checks; no cached row authorizes a capability.
use super::*;

pub(super) const NONE: u32 = u32::MAX;

fn allocation() -> ProductionSemanticKirErrorV1 {
    ProductionSemanticKirErrorV1::AllocationFailure {
        resource: ProductionSemanticKirResourceV1::AnalysisWork,
    }
}

// Cumulative debits retain the old allocation's charge through growth. Charge
// requested new capacity and relocation before allocation; any allocator excess
// is charged immediately, before the vector can be published or consumed.
pub(super) fn reserve<T: Copy>(
    values: &mut Vec<T>,
    capacity: usize,
    charge: &mut dyn FnMut(usize) -> Result<()>,
) -> Result<()> {
    if capacity <= values.capacity() {
        return Ok(());
    }
    let words = std::mem::size_of::<T>().div_ceil(std::mem::size_of::<usize>());
    let work = capacity
        .checked_add(values.len())
        .and_then(|n| n.checked_mul(words))
        .ok_or_else(mismatch)?;
    charge(work)?;
    values
        .try_reserve_exact(capacity - values.len())
        .map_err(|_| allocation())?;
    let extra = values
        .capacity()
        .checked_sub(capacity)
        .ok_or_else(mismatch)?;
    charge(extra.checked_mul(words).ok_or_else(mismatch)?)
}

pub(super) fn push<T: Copy>(
    values: &mut Vec<T>,
    value: T,
    charge: &mut dyn FnMut(usize) -> Result<()>,
) -> Result<()> {
    charge(1)?;
    if values.len() == values.capacity() {
        let next = values.len().checked_add(1).ok_or_else(mismatch)?;
        let capacity = values
            .capacity()
            .checked_mul(2)
            .ok_or_else(mismatch)?
            .max(next);
        reserve(values, capacity, charge)?;
    }
    values.push(value);
    Ok(())
}

pub(super) fn filled<T: Copy>(
    len: usize,
    value: T,
    charge: &mut dyn FnMut(usize) -> Result<()>,
) -> Result<Vec<T>> {
    let mut result = Vec::new();
    reserve(&mut result, len, charge)?;
    charge(len)?;
    result.resize(len, value);
    Ok(result)
}

pub(super) struct MatrixSites<'a> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    view: &'a SemanticExpandedRootV1,
    heads: Vec<u32>,
    rows: Vec<(u32, u32)>, // Expanded block, next row; preserve source order.
}

pub(super) struct MatrixBlocks<'a> {
    rows: &'a [(u32, u32)],
    next: u32,
}

impl Iterator for MatrixBlocks<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        if self.next == NONE {
            return None;
        }
        let (block, next) = self.rows[self.next as usize];
        self.next = next;
        Some(block)
    }
}

impl<'a> MatrixSites<'a> {
    pub(super) fn new(
        owner: &'a ProductionSemanticSsaOwnerV1,
        view: &'a SemanticExpandedRootV1,
        graph: &mut Graph<'a>,
    ) -> Result<Self> {
        graph.charge(
            std::mem::size_of::<Self>().div_ceil(std::mem::size_of::<usize>())
                + std::mem::size_of::<Vec<u32>>().div_ceil(std::mem::size_of::<usize>()),
        )?;
        if !std::ptr::eq(graph.body, view.body())
            || view.block_origins().len() != view.body().blocks().len()
        {
            return Err(mismatch());
        }
        let mut heads = filled(view.instances().len(), NONE, &mut |n| graph.charge(n))?;
        let mut tails = filled(heads.len(), NONE, &mut |n| graph.charge(n))?;
        let mut rows: Vec<(u32, u32)> = Vec::new();
        for (block, origin) in view.block_origins().iter().enumerate() {
            graph.charge(1)?;
            if origin.terminator() != SemanticExpandedTerminatorOriginV1::Source {
                continue;
            }
            let SemanticTerminatorKindV1::Call(call) =
                view.body().blocks()[block].terminator().kind()
            else {
                continue;
            };
            if !matches!(owner.source_semantic().callables().get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic { operation:
                    SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract }, .. })
                if matches!(contract.operation(), SemanticExecutionCapabilityOperationV1::MatrixAccess { .. }))
            {
                continue;
            }
            let block = u32::try_from(block).map_err(|_| mismatch())?;
            let instance = origin.instance().index() as usize;
            append(
                &mut heads,
                &mut tails,
                &mut rows,
                instance,
                block,
                &mut |n| graph.charge(n),
            )?;
        }
        Ok(Self {
            owner,
            view,
            heads,
            rows,
        })
    }

    pub(super) fn blocks(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        view: &SemanticExpandedRootV1,
        instance: SemanticCallInstanceIdV1,
        graph: &mut Graph<'_>,
    ) -> Result<MatrixBlocks<'_>> {
        graph.charge(
            4 + std::mem::size_of::<MatrixBlocks<'_>>().div_ceil(std::mem::size_of::<usize>()),
        )?;
        if !std::ptr::eq(owner, self.owner)
            || !std::ptr::eq(view, self.view)
            || !std::ptr::eq(graph.body, view.body())
        {
            return Err(mismatch());
        }
        let head = *self
            .heads
            .get(instance.index() as usize)
            .ok_or_else(mismatch)?;
        Ok(MatrixBlocks {
            rows: &self.rows,
            next: head,
        })
    }
}

fn append(
    heads: &mut [u32],
    tails: &mut [u32],
    rows: &mut Vec<(u32, u32)>,
    instance: usize,
    block: u32,
    charge: &mut dyn FnMut(usize) -> Result<()>,
) -> Result<()> {
    charge(3)?;
    let head = heads.get_mut(instance).ok_or_else(mismatch)?;
    let tail = tails.get_mut(instance).ok_or_else(mismatch)?;
    let row = u32::try_from(rows.len())
        .ok()
        .filter(|&n| n != NONE)
        .ok_or_else(mismatch)?;
    push(rows, (block, NONE), charge)?;
    if *tail == NONE {
        *head = row;
    } else {
        rows.get_mut(*tail as usize).ok_or_else(mismatch)?.1 = row;
    }
    *tail = row;
    Ok(())
}

#[cfg(test)]
mod tests;
