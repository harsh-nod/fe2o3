fn scoped<T>(budget: &mut Budget<'_>, run: impl FnOnce(&mut Budget<'_>) -> Result<T>) -> Result<T> {
    let floor = budget.storage();
    let slot = budget as *const Budget<'_> as usize;
    let ledger: CanonicalKernelIrWorkLedgerIdentityV1 = budget.work_ledger_identity_v1();
    let mut panic = None;
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            panic = Some(payload);
            Err(Error::Panicked)
        }
    };
    if slot != budget as *const Budget<'_> as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage() < floor
    {
        // Private run has no external callback except the separately guarded CFG
        // scope. Never clean a substituted ledger if that guard reports misuse.
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(result))) {
            panic = Some(payload);
        }
        result = Err(Resource::Accounting.into());
    } else {
        let owned = budget.storage() - floor;
        if let Err(error) = budget.release_storage(owned) {
            drop(result);
            drop(panic);
            return Err(error.into());
        }
    }
    drop(panic);
    result
}

fn bytes<T>(capacity: usize) -> Result<usize> {
    capacity
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic.into())
}
fn vector<T>(capacity: usize, budget: &mut Budget<'_>) -> Result<Vec<T>> {
    budget.charge_work(2)?;
    let requested = bytes::<T>(capacity)?;
    budget.reserve_storage(requested)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| Resource::Allocation)?;
    reconcile_capacity::<T>(capacity, values.capacity(), budget)?;
    Ok(values)
}
fn reconcile_capacity<T>(requested: usize, capacity: usize, budget: &mut Budget<'_>) -> Result<()> {
    let actual = bytes::<T>(capacity)?;
    let requested = bytes::<T>(requested)?;
    budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
    Ok(())
}
fn filled<T: Clone>(count: usize, value: T, budget: &mut Budget<'_>) -> Result<Vec<T>> {
    let mut values = vector(count, budget)?;
    budget.charge_work(count)?;
    values.resize(count, value);
    Ok(values)
}
fn push<T: Clone>(
    values: &mut Vec<T>,
    value: T,
    limit: usize,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(2)?;
    count(
        "retained rows",
        values.len().checked_add(1).ok_or(Resource::Arithmetic)?,
        limit,
    )?;
    if values.len() == values.capacity() {
        let capacity = values
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .ok_or(Resource::Arithmetic)?
            .min(limit);
        let mut next = vector(capacity, budget)?;
        budget.charge_work(values.len())?;
        next.extend_from_slice(values);
        let old = std::mem::replace(values, next);
        let old_bytes = bytes::<T>(old.capacity())?;
        drop(old);
        budget.release_storage(old_bytes)?;
    }
    values.push(value);
    Ok(())
}
fn count(kind: &'static str, actual: usize, limit: usize) -> Result<()> {
    if actual > limit {
        Err(Error::InputLimit {
            kind,
            actual,
            limit,
        })
    } else {
        Ok(())
    }
}
fn input_limits(
    inventory: &Inventory<'_>,
    limits: CanonicalKirLoopLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(6)?;
    for (kind, actual, limit) in [
        ("functions", inventory.functions().len(), limits.functions),
        ("blocks", inventory.blocks().len(), limits.blocks),
        ("edges", inventory.edges().len(), limits.edges),
        (
            "definitions",
            inventory.definitions().len(),
            limits.definitions,
        ),
        (
            "operations",
            inventory.operations().len(),
            limits.operations,
        ),
    ] {
        count(kind, actual, limit)?;
    }
    Ok(())
}
fn block_index(inventory: &Inventory<'_>, block: Block, budget: &mut Budget<'_>) -> Result<usize> {
    budget.charge_work(4)?;
    let function = inventory
        .functions()
        .get(block.function.0 as usize)
        .ok_or(Error::InconsistentInventory)?;
    let index = function
        .blocks
        .start
        .checked_add(block.block as usize)
        .ok_or(Resource::Arithmetic)?;
    if index >= function.blocks.end
        || inventory
            .blocks()
            .get(index)
            .is_none_or(|r| r.coordinate != block)
    {
        return Err(Error::InconsistentInventory);
    }
    Ok(index)
}
fn operation<'a, 'g>(
    inventory: &'a Inventory<'g>,
    coordinate: Operation,
    budget: &mut Budget<'_>,
) -> Result<&'a crate::CanonicalKirOperationRefV1<'g>> {
    let block = block_index(inventory, coordinate.block, budget)?;
    budget.charge_work(3)?;
    let range = &inventory.blocks()[block].operations;
    let index = range
        .start
        .checked_add(coordinate.operation as usize)
        .ok_or(Resource::Arithmetic)?;
    inventory
        .operations()
        .get(index)
        .filter(|row| index < range.end && row.coordinate == coordinate)
        .ok_or(Error::InconsistentInventory)
}

// Immutable incidence indices reference actual edge rows; they are neither an
// editable graph nor an alternate executable representation.
struct Incidence {
    incoming: Vec<usize>,
    offsets: Vec<usize>,
    sources: Vec<usize>,
    targets: Vec<usize>,
}
impl Incidence {
    fn build(inventory: &Inventory<'_>, budget: &mut Budget<'_>) -> Result<Self> {
        let b = inventory.blocks().len();
        let e = inventory.edges().len();
        let mut offsets = filled(
            b.checked_add(1).ok_or(Resource::Arithmetic)?,
            0usize,
            budget,
        )?;
        let mut sources = vector(e, budget)?;
        let mut targets = vector(e, budget)?;
        for edge in inventory.edges() {
            budget.charge_work(3)?;
            let source = block_index(inventory, edge.coordinate.source, budget)?;
            let target = block_index(inventory, edge.target, budget)?;
            offsets[target + 1] = offsets[target + 1]
                .checked_add(1)
                .ok_or(Resource::Arithmetic)?;
            sources.push(source);
            targets.push(target);
        }
        for index in 0..b {
            budget.charge_work(2)?;
            offsets[index + 1] = offsets[index + 1]
                .checked_add(offsets[index])
                .ok_or(Resource::Arithmetic)?;
        }
        let mut cursor = vector(b, budget)?;
        budget.charge_work(b)?;
        cursor.extend_from_slice(&offsets[..b]);
        let mut incoming = filled(e, 0usize, budget)?;
        for (edge, target) in targets.iter().copied().enumerate() {
            budget.charge_work(3)?;
            incoming[cursor[target]] = edge;
            cursor[target] += 1;
        }
        Ok(Self {
            incoming,
            offsets,
            sources,
            targets,
        })
    }
    fn predecessors(&self, block: usize) -> &[usize] {
        &self.incoming[self.offsets[block]..self.offsets[block + 1]]
    }
}

struct Scratch {
    reachable: Vec<bool>,
    avoiding: Vec<bool>,
    members: Vec<bool>,
    backedges: Vec<bool>,
    headers: Vec<bool>,
    stack: Vec<usize>,
}
impl Scratch {
    fn new(blocks: usize, edges: usize, budget: &mut Budget<'_>) -> Result<Self> {
        Ok(Self {
            reachable: filled(blocks, false, budget)?,
            avoiding: filled(blocks, false, budget)?,
            members: filled(blocks, false, budget)?,
            backedges: filled(edges, false, budget)?,
            headers: filled(blocks, false, budget)?,
            stack: vector(blocks, budget)?,
        })
    }
}

fn fixed_integer_bits(constant: &Constant) -> Option<(ScalarType, u64)> {
    Some(match *constant {
        Constant::I8(v) => (ScalarType::I8, v as u8 as u64),
        Constant::I16(v) => (ScalarType::I16, v as u16 as u64),
        Constant::I32(v) => (ScalarType::I32, v as u32 as u64),
        Constant::I64(v) => (ScalarType::I64, v as u64),
        Constant::U8(v) => (ScalarType::U8, v as u64),
        Constant::U16(v) => (ScalarType::U16, v as u64),
        Constant::U32(v) => (ScalarType::U32, v as u64),
        Constant::U64(v) => (ScalarType::U64, v),
        _ => return None,
    })
}
