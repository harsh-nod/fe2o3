//! Retain allocation origins from the completed ordered audit. This is a
//! physical projection aid, not source authority or a new active-loan graph.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReusablePhasePhysicalOriginV1 {
    allocation: ValueId,
    element: ExecutionTypeIdentityV1,
    layout: ExecutionElementLayoutV1,
    elements: u64,
}
impl ReusablePhasePhysicalOriginV1 {
    pub const fn allocation(&self) -> ValueId {
        self.allocation
    }
    pub const fn element(&self) -> ExecutionTypeIdentityV1 {
        self.element
    }
    pub const fn layout(&self) -> ExecutionElementLayoutV1 {
        self.layout
    }
    pub const fn elements(&self) -> u64 {
        self.elements
    }
}

#[derive(Debug)]
pub struct ReusablePhasePhysicalResultV1<'a> {
    block: BlockId,
    operation_index: usize,
    operation: &'a Operation,
    result: &'a crate::ValueDef,
    owner: ValueId,
    storage: Option<ReusablePhasePhysicalOriginV1>,
    projected: bool,
}
impl<'a> ReusablePhasePhysicalResultV1<'a> {
    pub const fn block(&self) -> BlockId {
        self.block
    }
    pub const fn operation_index(&self) -> usize {
        self.operation_index
    }
    pub const fn operation(&self) -> &'a Operation {
        self.operation
    }
    pub const fn result(&self) -> &'a crate::ValueDef {
        self.result
    }
    pub const fn owner(&self) -> ValueId {
        self.owner
    }
    pub const fn storage(&self) -> Option<ReusablePhasePhysicalOriginV1> {
        self.storage
    }
    /// Storage loans carry an origin for exact End replay but have no physical
    /// ABI value. Only source LDS handles materialize as pointer aliases.
    pub fn materializes_pointer(&self) -> bool {
        matches!(&self.result.ty, Type::ExecutionCapability(c)
            if matches!(c.role, Role::Lds { .. } | Role::ReusableLds { .. }))
    }
}

/// Borrowed from the exact audited function; cannot be cloned or decoded.
/// Source, target, ordinary KIR verification, and memory proofs remain with
/// their existing production owners. No storage, load, or barrier is emitted.
#[derive(Debug)]
pub struct ReusablePhasePhysicalProjectionV1<'a> {
    function: &'a Function,
    results: Vec<ReusablePhasePhysicalResultV1<'a>>,
    usage: ReusablePhaseCheckUsageV1,
    work_limit: usize,
    storage_limit: usize,
}
impl<'a> ReusablePhasePhysicalProjectionV1<'a> {
    pub const fn function(&self) -> &'a Function {
        self.function
    }
    pub fn results(&self) -> &[ReusablePhasePhysicalResultV1<'a>] {
        &self.results
    }
    pub const fn usage(&self) -> ReusablePhaseCheckUsageV1 {
        self.usage
    }
    /// Exact coordinate lookup, charged to the same cumulative work ceiling as
    /// the ordered pass. Results stay borrowed from the immutable source graph.
    pub fn operation_results(
        &mut self,
        block: BlockId,
        operation_index: usize,
    ) -> Result<&[ReusablePhasePhysicalResultV1<'a>]> {
        let search = 2usize
            .checked_mul(1 + self.results.len().max(1).ilog2() as usize)
            .ok_or(Error::Overflow)?;
        self.charge(search)?;
        let key = (block, operation_index);
        let start = self
            .results
            .partition_point(|r| (r.block, r.operation_index) < key);
        let end = self
            .results
            .partition_point(|r| (r.block, r.operation_index) <= key);
        if start == end {
            return Err(Error::MissingProducer);
        }
        self.charge(end - start)?;
        if self.results[start..end].iter().any(|r| r.projected) {
            return Err(Error::DuplicateConsumption);
        }
        for row in &mut self.results[start..end] {
            row.projected = true;
        }
        Ok(&self.results[start..end])
    }
    /// Consumer map traversal uses this same remaining budget, rather than
    /// restarting the verifier ceiling for every logical operation.
    pub fn charge_adapter_work(&mut self, work: usize) -> Result<()> {
        self.charge(work)
    }
    /// Charge concurrent consumer buffers in addition to this retained audit.
    /// The caller supplies actual allocated capacity, not merely entry count.
    pub fn observe_adapter_peak(&mut self, additional_bytes: usize) -> Result<()> {
        let peak = self.retained_bytes().checked_add(additional_bytes).ok_or(Error::Overflow)?;
        if peak > self.storage_limit { return Err(Error::StorageLimit); }
        self.usage.peak_temporary_bytes = self.usage.peak_temporary_bytes.max(peak);
        Ok(())
    }
    pub fn finish(mut self) -> Result<ReusablePhaseCheckUsageV1> {
        self.charge(self.results.len())?;
        if self.results.iter().any(|r| !r.projected) {
            return Err(Error::MissingProducer);
        }
        Ok(self.usage)
    }
    fn charge(&mut self, work: usize) -> Result<()> {
        self.usage.work = self.usage.work.checked_add(work).ok_or(Error::Overflow)?;
        if self.usage.work > self.work_limit {
            return Err(Error::WorkLimit);
        }
        Ok(())
    }
    pub fn retained_bytes(&self) -> usize {
        // Construction charged this exact product, including excess capacity.
        self.results.capacity() * std::mem::size_of::<ReusablePhasePhysicalResultV1<'a>>()
    }
}

/// Run with the existing exact function CFG after ordinary KIR verification.
/// Uses the same verifier pass and all its negatives; no separate ancestry
/// reconstruction or limit increase is introduced by physical projection.
pub fn project_reusable_phase_function_v1<'a>(
    function: &'a Function,
    cfg: &IndexedControlFlow,
    limits: ReusablePhaseCheckLimitsV1,
) -> Result<ReusablePhasePhysicalProjectionV1<'a>> {
    let (usage, results) = run(function, cfg, limits, collect)?;
    Ok(ReusablePhasePhysicalProjectionV1 {
        function,
        results,
        usage,
        work_limit: limits.work.min(ReusablePhaseCheckLimitsV1::DEFAULT.work),
        storage_limit: limits.temporary_bytes.min(ReusablePhaseCheckLimitsV1::DEFAULT.temporary_bytes),
    })
}

fn collect<'a>(
    nodes: &[Node<'a>],
    values: &[Value<'a>],
    b: &mut Budget,
) -> Result<Vec<ReusablePhasePhysicalResultV1<'a>>> {
    let mut count = 0usize;
    for node in nodes {
        b.work(1)?;
        if matches!(node.op.kind, OperationKind::ReusablePhase(_)) {
            count = count
                .checked_add(node.op.results.len())
                .ok_or(Error::Overflow)?;
        }
    }
    let mut results = b.reserve(count)?;
    for node in nodes {
        b.work(1)?;
        if !matches!(node.op.kind, OperationKind::ReusablePhase(_)) {
            continue;
        }
        for result in &node.op.results {
            b.work(1)?;
            let f = fact(values, result.id, b)?;
            let storage = if let Some(allocation) = f.allocation {
                let value = &values[index(values, allocation, b)?];
                let allocation_op = nodes.get(value.producer).ok_or(Error::MissingProducer)?.op;
                let OperationKind::ExecutionCapability(contract) = &allocation_op.kind else {
                    return Err(Error::WrongStorage);
                };
                let (element, layout, elements) = match contract.operation {
                    Exec::LdsAllocate {
                        element,
                        layout,
                        elements,
                        ..
                    }
                    | Exec::LdsAllocateBorrowed {
                        element,
                        layout,
                        elements,
                        ..
                    } => (element, layout, elements),
                    _ => return Err(Error::WrongStorage),
                };
                if allocation_op.results.len() != 1
                    || allocation_op.results[0].id != allocation
                    || !matches!(value.ty, Type::ExecutionCapability(c) if c.role == (Role::Lds {
                        element, layout, elements, state: crate::ExecutionLdsStateV1::Uninitialized }))
                {
                    return Err(Error::WrongStorage);
                }
                Some(ReusablePhasePhysicalOriginV1 {
                    allocation,
                    element,
                    layout,
                    elements,
                })
            } else {
                None
            };
            let requires_storage = match &result.ty {
                Type::ExecutionCapability(c) => match c.role {
                    Role::Lds { .. } | Role::ReusableLds { .. } => true,
                    Role::Workgroup | Role::ReusableWorkgroup | Role::ReusablePhaseCompletion => {
                        false
                    }
                    _ => return Err(Error::LocalContract),
                },
                Type::ReusablePhaseToken(t) => matches!(
                    t.role,
                    ReusablePhaseTokenRoleV1::StorageLoan(_)
                        | ReusablePhaseTokenRoleV1::ClosedStorage(_)
                ),
                _ => return Err(Error::LocalContract),
            };
            if requires_storage != storage.is_some() {
                return Err(Error::WrongStorage);
            }
            if results.len() == count {
                return Err(Error::Overflow);
            }
            results.push(ReusablePhasePhysicalResultV1 {
                block: node.at.block,
                operation_index: node.at.operation,
                operation: node.op,
                result,
                owner: f.owner,
                storage,
                projected: false,
            });
        }
    }
    if results.len() != count {
        return Err(Error::MissingProducer);
    }
    Ok(results)
}
