//! Bounded summaries for finite, noncollective defined calls. No helper ABI
//! uniformity is assumed: caller operands still taint every call result.
use super::*;
use fe2o3_kernel_ir::FunctionRole;

#[derive(Clone, Copy, Default)]
struct Summary {
    admitted: bool,
    varying: bool,
    complete: bool,
}

pub(super) struct Summaries {
    targets: Vec<Option<usize>>,
    functions: Vec<Summary>,
}

fn block_target(
    inventory: &CanonicalKirInventoryV18<'_>,
    function: &CanonicalKirFunctionRefV1<'_>,
    edge: usize,
) -> Result<usize> {
    let target = inventory.edges().get(edge).ok_or(Error::Inventory)?.target;
    if target.function != function.coordinate {
        return Err(Error::Inventory);
    }
    let block = function
        .blocks
        .start
        .checked_add(target.block as usize)
        .ok_or(Resource::Arithmetic)?;
    if !function.blocks.contains(&block) {
        return Err(Error::Inventory);
    }
    Ok(block)
}

impl Summaries {
    pub(super) fn derive(
        inventory: &CanonicalKirInventoryV18<'_>,
        meter: &mut Meter<'_, '_, Error>,
    ) -> Result<Self> {
        meter.reserve(size_of::<Self>() + 9 * size_of::<Vec<usize>>())?;
        let mut functions = filled(meter, inventory.functions().len(), Summary::default())?;
        let mut targets = filled(meter, inventory.operations().len(), None)?;
        let mut reachable = filled(meter, inventory.blocks().len(), false)?;
        let mut incoming = filled(meter, inventory.blocks().len(), 0usize)?;
        let mut order = meter.table(inventory.blocks().len())?.0;
        let mut pending = filled(meter, functions.len(), 0usize)?;
        let mut degrees = filled(meter, functions.len(), 0usize)?;
        let mut edges = meter.table(inventory.calls().len())?.0;

        // Reachability and Kahn order are local to each body. Physical function
        // or block order is never treated as a dependency order.
        for function in inventory.functions() {
            meter.work(1)?;
            if function.blocks.is_empty() {
                continue;
            }
            order.clear();
            reachable[function.blocks.start] = true;
            meter.push(&mut order, function.blocks.start)?;
            let mut head = 0;
            while head < order.len() {
                meter.work(1)?;
                let block = order[head];
                head += 1;
                for edge in inventory.blocks()[block].edges.clone() {
                    meter.work(1)?;
                    let target = block_target(inventory, function, edge)?;
                    incoming[target] = incoming[target]
                        .checked_add(1)
                        .ok_or(Resource::Arithmetic)?;
                    if !reachable[target] {
                        reachable[target] = true;
                        meter.push(&mut order, target)?;
                    }
                }
            }
            let live = order.len();
            order.clear();
            for block in function.blocks.clone() {
                meter.work(1)?;
                if reachable[block] && incoming[block] == 0 {
                    meter.push(&mut order, block)?;
                }
            }
            head = 0;
            while head < order.len() {
                meter.work(1)?;
                let block = order[head];
                head += 1;
                for edge in inventory.blocks()[block].edges.clone() {
                    meter.work(1)?;
                    let target = block_target(inventory, function, edge)?;
                    incoming[target] = incoming[target].checked_sub(1).ok_or(Error::Inventory)?;
                    if incoming[target] == 0 {
                        meter.push(&mut order, target)?;
                    }
                }
            }
            let summary = &mut functions[function.coordinate.0 as usize];
            summary.admitted =
                function.function.role == FunctionRole::InternalHelper && order.len() == live;
            for block in function.blocks.clone() {
                meter.work(1)?;
                if !reachable[block] {
                    continue;
                }
                let block = &inventory.blocks()[block];
                if block.edges.is_empty() && !matches!(block.terminator, Terminator::Return { .. })
                {
                    summary.admitted = false;
                }
                for index in block.operations.clone() {
                    meter.work(1)?;
                    let operation = &inventory.operations()[index];
                    match &operation.operation.kind {
                        Kind::Call { .. } => {}
                        Kind::Execution(_) | Kind::Barrier(_) | Kind::WorkgroupBarrier(_) => {
                            summary.admitted = false;
                        }
                        kind => match varying_seed(kind, operation.coordinate) {
                            Ok(varying) => summary.varying |= varying,
                            Err(Error::UnsupportedArrival(_)) => summary.admitted = false,
                            Err(error) => return Err(error),
                        },
                    }
                }
            }
        }

        // Resolve only through the authenticated inventory, never a callee name
        // heuristic. Duplicate call sites retain distinct dependency edges.
        for call in inventory.calls() {
            meter.work(1)?;
            let caller = call.coordinate.block.function.0 as usize;
            let function = inventory.functions().get(caller).ok_or(Error::Inventory)?;
            let block = function
                .blocks
                .start
                .checked_add(call.coordinate.block.block as usize)
                .ok_or(Resource::Arithmetic)?;
            if !function.blocks.contains(&block) {
                return Err(Error::Inventory);
            }
            let operations = inventory.blocks()[block].operations.clone();
            let operation = operations
                .start
                .checked_add(call.coordinate.operation as usize)
                .ok_or(Resource::Arithmetic)?;
            if !operations.contains(&operation)
                || inventory.operations()[operation].coordinate != call.coordinate
                || !std::ptr::eq(inventory.operations()[operation].operation, call.operation)
            {
                return Err(Error::Inventory);
            }
            if !reachable[block] {
                continue;
            }
            let Some(target) = call.target.map(|target| target.0 as usize) else {
                functions[caller].admitted = false;
                continue;
            };
            if target >= functions.len() || targets[operation].replace(target).is_some() {
                return Err(Error::Inventory);
            }
            pending[caller] = pending[caller].checked_add(1).ok_or(Resource::Arithmetic)?;
            degrees[target] = degrees[target].checked_add(1).ok_or(Resource::Arithmetic)?;
            meter.push(&mut edges, (target, caller))?;
        }
        let mut starts = meter
            .table(functions.len().checked_add(1).ok_or(Resource::Arithmetic)?)?
            .0;
        meter.push(&mut starts, 0usize)?;
        for &degree in &degrees {
            meter.work(1)?;
            let next = starts
                .last()
                .copied()
                .ok_or(Error::Inventory)?
                .checked_add(degree)
                .ok_or(Resource::Arithmetic)?;
            meter.push(&mut starts, next)?;
        }
        let mut callers = filled(meter, edges.len(), 0usize)?;
        for degree in &mut degrees {
            meter.work(1)?;
            *degree = 0;
        }
        for (target, caller) in edges {
            meter.work(1)?;
            let at = starts[target]
                .checked_add(degrees[target])
                .ok_or(Resource::Arithmetic)?;
            if at >= starts[target + 1] {
                return Err(Error::Inventory);
            }
            callers[at] = caller;
            degrees[target] += 1;
        }
        let mut ready = meter.table(functions.len())?.0;
        for (function, &count) in pending.iter().enumerate() {
            meter.work(1)?;
            if count == 0 {
                meter.push(&mut ready, function)?;
            }
        }
        let mut head = 0;
        while head < ready.len() {
            meter.work(1)?;
            let callee = ready[head];
            head += 1;
            functions[callee].complete = true;
            let summary = functions[callee];
            for &caller in &callers[starts[callee]..starts[callee + 1]] {
                meter.work(1)?;
                functions[caller].admitted &= summary.admitted;
                functions[caller].varying |= summary.varying;
                pending[caller] = pending[caller].checked_sub(1).ok_or(Error::Inventory)?;
                if pending[caller] == 0 {
                    meter.push(&mut ready, caller)?;
                }
            }
        }
        // Recursive SCCs and their dependents remain incomplete and refuse.
        Ok(Self { targets, functions })
    }

    pub(super) fn varying(
        &self,
        operation: usize,
        site: Site,
        meter: &mut Meter<'_, '_, Error>,
    ) -> Result<bool> {
        meter.work(2)?;
        let target = self
            .targets
            .get(operation)
            .copied()
            .flatten()
            .ok_or(Error::UnsupportedArrival(site))?;
        let summary = self.functions.get(target).ok_or(Error::Inventory)?;
        if !summary.complete || !summary.admitted {
            return Err(Error::UnsupportedArrival(site));
        }
        Ok(summary.varying)
    }
}
