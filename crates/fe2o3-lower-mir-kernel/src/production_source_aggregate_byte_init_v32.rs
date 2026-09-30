// Mandatory all-endpoint definite initialization in the stable original
// allocation namespace. Byte boundaries are sparse; aggregate sizes are never
// expanded. Unsupported allocation uses remain explicit census obligations.
type AggregateByteInventoryV32<'a> = fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>;
type AggregateByteQueueV32 = fe2o3_kernel_analysis::CanonicalKirPrivateDataflowQueueV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AggregateByteBoundaryV32 {
    allocation: usize,
    offset: u64,
}

impl AggregateByteBoundaryV32 {
    fn order(self) -> [usize; 3] {
        [
            self.allocation,
            (self.offset >> 32) as usize,
            (self.offset & u64::from(u32::MAX)) as usize,
        ]
    }
}

#[derive(Clone)]
struct AggregateByteLeafV32 {
    allocation: usize,
    cells: std::ops::Range<usize>,
}

struct AggregateBytePlanV32 {
    allocations: Vec<AggregateOperationV30>,
    boundaries: Vec<AggregateByteBoundaryV32>,
    allocation_cells: Vec<std::ops::Range<usize>>,
    leaves: Vec<AggregateByteLeafV32>,
}

#[derive(Clone, Copy)]
enum AggregateByteEventV32 {
    // An unsupported event is not credited with a write, reset or safe read.
    Unresolved,
    Allocate(usize),
    Project(usize),
    Read(usize),
    Write(usize),
}

struct AggregateByteBlockV32 {
    entry: bool,
    operations: std::ops::Range<usize>,
    edges: std::ops::Range<usize>,
}

struct AggregateByteFlowV32 {
    blocks: Vec<AggregateByteBlockV32>,
    edges: Vec<usize>,
    events: Vec<AggregateByteEventV32>,
    allocation_cells: Vec<std::ops::Range<usize>>,
    leaves: Vec<AggregateByteLeafV32>,
}

fn aggregate_byte_error_v32(detail: &'static str) -> ProductionAggregateSourceErrorV30 {
    ProductionSourceOwnedViewErrorV18::Binding(detail).into()
}

fn aggregate_byte_queue_error_v32(
    error: fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1,
) -> ProductionAggregateSourceErrorV30 {
    ProductionSourceOwnedViewErrorV18::from(error).into()
}

fn aggregate_byte_vec_v32<T>(
    length: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<T>, ProductionAggregateSourceErrorV30> {
    source_reference_emission_vec_v29(length, budget)
        .map_err(source_argument_error_v18)
        .map_err(Into::into)
}

fn aggregate_byte_order_v32(allocation: AggregateOperationV30) -> (u32, u32, u32) {
    (
        allocation.block.function.0,
        allocation.block.block,
        allocation.operation,
    )
}

fn aggregate_byte_payload_v32(
    layout: &fe2o3_kernel_ir::StorageLayoutV1,
    ty: AggregateMemoryLeafV31,
) -> Result<u64, ProductionAggregateSourceErrorV30> {
    use fe2o3_kernel_ir::StorageLayoutKindV1 as Layout;
    let width = match (&layout.kind, ty) {
        (Layout::Scalar(actual), AggregateMemoryLeafV31::Scalar(expected))
            if *actual == expected =>
        {
            match actual.bit_width() {
                Some(bits) => u64::from(bits.div_ceil(8)),
                None if *actual == ScalarType::Index
                    && matches!(layout.size, 1 | 2 | 4 | 8 | 16) =>
                {
                    layout.size
                }
                None => {
                    return Err(aggregate_byte_error_v32(
                        "private byte scalar payload width",
                    ));
                }
            }
        }
        (Layout::Vector(actual), AggregateMemoryLeafV31::Vector(expected))
            if *actual == expected =>
        {
            actual
                .byte_width()
                .map(u64::from)
                .ok_or_else(|| aggregate_byte_error_v32("private byte vector payload width"))?
        }
        _ => return Err(aggregate_byte_error_v32("private byte exact typed layout")),
    };
    if width == 0 || width > layout.size {
        return Err(aggregate_byte_error_v32(
            "private byte payload exceeds storage extent",
        ));
    }
    Ok(width)
}

impl AggregateBytePlanV32 {
    fn allocation(
        &self,
        original: AggregateOperationV30,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionAggregateSourceErrorV30> {
        budget.charge_work(self.allocations.len().checked_ilog2().unwrap_or(0) as usize + 3)?;
        self.allocations
            .binary_search_by_key(&aggregate_byte_order_v32(original), |row| {
                aggregate_byte_order_v32(*row)
            })
            .map_err(|_| aggregate_byte_error_v32("private byte original allocation is absent"))
    }

    fn build(
        memory: &ProductionAggregateMemoryChainV31,
        original: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionAggregateSourceErrorV30> {
        let first = memory
            .endpoints
            .first()
            .ok_or(ArgumentResourceV1::Accounting)?;
        let mut plan = Self {
            allocations: aggregate_byte_vec_v32(first.allocations.len(), budget)?,
            boundaries: aggregate_byte_vec_v32(argument_product_v1(memory.keys.len(), 2)?, budget)?,
            allocation_cells: aggregate_byte_vec_v32(first.allocations.len(), budget)?,
            leaves: aggregate_byte_vec_v32(memory.keys.len(), budget)?,
        };
        for row in &memory.allocations[first.allocations.clone()] {
            budget.charge_work(3)?;
            if row.original != row.actual
                || plan.allocations.last().is_some_and(|prior| {
                    aggregate_byte_order_v32(*prior) >= aggregate_byte_order_v32(row.original)
                })
            {
                return Err(aggregate_byte_error_v32(
                    "private byte original allocation order",
                ));
            }
            plan.allocations.push(row.original);
        }
        for key in &memory.keys {
            budget.charge_work(16)?;
            let allocation = plan.allocation(key.allocation, budget)?;
            let layout = original
                .module()
                .storage_layouts
                .get(key.layout.0 as usize)
                .ok_or_else(|| aggregate_byte_error_v32("private byte original layout"))?;
            let payload = aggregate_byte_payload_v32(layout, key.ty)?;
            let root = original
                .module()
                .functions
                .get(key.allocation.block.function.0 as usize)
                .and_then(|function| function.body.as_ref())
                .and_then(|body| body.blocks.get(key.allocation.block.block as usize))
                .and_then(|block| block.operations.get(key.allocation.operation as usize))
                .ok_or_else(|| {
                    aggregate_byte_error_v32("private byte original allocation occurrence")
                })?;
            let OperationKind::Alloca {
                element: Type::StorageObject(root_layout),
                ..
            } = &root.kind
            else {
                return Err(aggregate_byte_error_v32(
                    "private byte original allocation layout",
                ));
            };
            let root_layout = original
                .module()
                .storage_layouts
                .get(root_layout.0 as usize)
                .ok_or_else(|| {
                    aggregate_byte_error_v32("private byte original allocation layout")
                })?;
            let extent_end = key
                .offset
                .checked_add(layout.size)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            if extent_end > root_layout.size {
                return Err(aggregate_byte_error_v32(
                    "private byte leaf storage exceeds original allocation",
                ));
            }
            let end = key
                .offset
                .checked_add(payload)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            plan.boundaries.push(AggregateByteBoundaryV32 {
                allocation,
                offset: key.offset,
            });
            plan.boundaries.push(AggregateByteBoundaryV32 {
                allocation,
                offset: end,
            });
        }
        private_array_heapsort_v1(
            &mut plan.boundaries,
            |row| row.order(),
            &mut SourceCorrespondenceWorkV18(budget),
            || ArgumentResourceV1::Arithmetic.into(),
        )?;
        let mut count = 0;
        for at in 0..plan.boundaries.len() {
            budget.charge_work(2)?;
            if count == 0 || plan.boundaries[count - 1] != plan.boundaries[at] {
                plan.boundaries[count] = plan.boundaries[at];
                count += 1;
            }
        }
        plan.boundaries.truncate(count);
        let mut at = 0;
        for allocation in 0..plan.allocations.len() {
            budget.charge_work(1)?;
            let start = at;
            while at < plan.boundaries.len() && plan.boundaries[at].allocation == allocation {
                budget.charge_work(1)?;
                at += 1;
            }
            // The final boundary is a zero-width sentinel, never a readable cell.
            plan.allocation_cells
                .push(start..at.saturating_sub(usize::from(at != start)));
        }
        if at != plan.boundaries.len() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        for key in &memory.keys {
            budget.charge_work(3)?;
            let allocation = plan.allocation(key.allocation, budget)?;
            let size = aggregate_byte_payload_v32(
                &original.module().storage_layouts[key.layout.0 as usize],
                key.ty,
            )?;
            let mut boundary = |offset| {
                budget
                    .charge_work(plan.boundaries.len().checked_ilog2().unwrap_or(0) as usize + 3)?;
                plan.boundaries
                    .binary_search_by_key(&(allocation, offset), |row| (row.allocation, row.offset))
                    .map_err(|_| aggregate_byte_error_v32("private byte leaf boundary is absent"))
            };
            let start = boundary(key.offset)?;
            let end = boundary(
                key.offset
                    .checked_add(size)
                    .ok_or(ArgumentResourceV1::Arithmetic)?,
            )?;
            if start >= end
                || !plan.allocation_cells[allocation].contains(&start)
                || end > plan.allocation_cells[allocation].end
            {
                return Err(aggregate_byte_error_v32(
                    "private byte leaf escapes allocation intervals",
                ));
            }
            plan.leaves.push(AggregateByteLeafV32 {
                allocation,
                cells: start..end,
            });
        }
        Ok(plan)
    }
}

impl AggregateByteFlowV32 {
    fn derive(
        inventory: &AggregateByteInventoryV32<'_>,
        function: &fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>,
        allocations: &[AggregateMemoryAllocationBindingV31],
        memory: &ProductionAggregateMemoryChainV31,
        endpoint: usize,
        plan: &AggregateBytePlanV32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionAggregateSourceErrorV30> {
        let boundary = memory
            .endpoints
            .get(endpoint)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if boundary.owner != std::ptr::from_ref(inventory.owner()) as usize
            || boundary.events.len() != inventory.operations().len()
        {
            return Err(aggregate_byte_error_v32(
                "private byte exact endpoint owner and operation census",
            ));
        }
        let mut local = aggregate_byte_vec_v32(allocations.len(), budget)?;
        for row in allocations {
            budget.charge_work(2)?;
            if row.actual.block.function != function.coordinate {
                return Err(aggregate_byte_error_v32(
                    "private byte allocation function differs",
                ));
            }
            if row.closed_uses == Some(true) {
                local.push(plan.allocation(row.original, budget)?);
            }
        }
        private_array_heapsort_v1(
            &mut local,
            |row| [*row],
            &mut SourceCorrespondenceWorkV18(budget),
            || ArgumentResourceV1::Arithmetic.into(),
        )?;
        let mut flow = Self {
            blocks: aggregate_byte_vec_v32(function.blocks.len(), budget)?,
            edges: aggregate_byte_vec_v32(function.edges.len(), budget)?,
            events: aggregate_byte_vec_v32(function.operations.len(), budget)?,
            allocation_cells: aggregate_byte_vec_v32(local.len(), budget)?,
            leaves: aggregate_byte_vec_v32(function.operations.len(), budget)?,
        };
        let mut cells = 0;
        for (index, &global) in local.iter().enumerate() {
            budget.charge_work(3)?;
            if index != 0 && local[index - 1] == global {
                return Err(aggregate_byte_error_v32(
                    "private byte allocation repeats in function",
                ));
            }
            let end = argument_sum_v1(&[cells, plan.allocation_cells[global].len()])?;
            flow.allocation_cells.push(cells..end);
            cells = end;
        }
        for row in &inventory.blocks()[function.blocks.clone()] {
            budget.charge_work(3)?;
            if row.coordinate.function != function.coordinate
                || row.operations.start < function.operations.start
                || row.operations.end > function.operations.end
                || row.edges.start < function.edges.start
                || row.edges.end > function.edges.end
            {
                return Err(aggregate_byte_error_v32(
                    "private byte block function range differs",
                ));
            }
            flow.blocks.push(AggregateByteBlockV32 {
                entry: row.coordinate.block == 0,
                operations: row.operations.start - function.operations.start
                    ..row.operations.end - function.operations.start,
                edges: row.edges.start - function.edges.start..row.edges.end - function.edges.start,
            });
        }
        for row in &inventory.edges()[function.edges.clone()] {
            budget.charge_work(2)?;
            if row.target.function != function.coordinate
                || row.coordinate.source.function != function.coordinate
                || row.target.block as usize >= function.blocks.len()
            {
                return Err(aggregate_byte_error_v32(
                    "private byte cross-function CFG edge",
                ));
            }
            flow.edges.push(row.target.block as usize);
        }
        for (at, actual) in inventory.operations()[function.operations.clone()]
            .iter()
            .enumerate()
        {
            use ProductionAggregateMemoryEventV31 as Event;
            budget.charge_work(4)?;
            let (coordinate, event) = memory
                .bound_event_v32(endpoint, function.operations.start + at)
                .ok_or(ArgumentResourceV1::Accounting)?;
            if coordinate != actual.coordinate || coordinate.block.function != function.coordinate {
                return Err(aggregate_byte_error_v32(
                    "private byte operation occurrence differs",
                ));
            }
            let selected = match event {
                Event::Unmodeled => AggregateByteEventV32::Unresolved,
                Event::Allocate { original } | Event::Project { original } => {
                    let allocation = plan.allocation(original, budget)?;
                    budget.charge_work(local.len().checked_ilog2().unwrap_or(0) as usize + 3)?;
                    match local.binary_search(&allocation) {
                        Err(_) => AggregateByteEventV32::Unresolved,
                        Ok(allocation) if matches!(event, Event::Allocate { .. }) => {
                            AggregateByteEventV32::Allocate(allocation)
                        }
                        Ok(allocation) => AggregateByteEventV32::Project(allocation),
                    }
                }
                Event::Read { leaf, .. } | Event::Write { leaf, .. } => {
                    let row = plan
                        .leaves
                        .get(leaf)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    budget.charge_work(local.len().checked_ilog2().unwrap_or(0) as usize + 3)?;
                    match local.binary_search(&row.allocation) {
                        Err(_) => AggregateByteEventV32::Unresolved,
                        Ok(allocation) => {
                            let global_start = plan.allocation_cells[row.allocation].start;
                            let local_start = flow.allocation_cells[allocation].start;
                            let start = row
                                .cells
                                .start
                                .checked_sub(global_start)
                                .ok_or(ArgumentResourceV1::Accounting)?;
                            let end = row
                                .cells
                                .end
                                .checked_sub(global_start)
                                .ok_or(ArgumentResourceV1::Accounting)?;
                            let cells = argument_sum_v1(&[local_start, start])?
                                ..argument_sum_v1(&[local_start, end])?;
                            if cells.end > flow.allocation_cells[allocation].end {
                                return Err(ArgumentResourceV1::Accounting.into());
                            }
                            let leaf = flow.leaves.len();
                            flow.leaves.push(AggregateByteLeafV32 { allocation, cells });
                            if matches!(event, Event::Read { .. }) {
                                AggregateByteEventV32::Read(leaf)
                            } else {
                                AggregateByteEventV32::Write(leaf)
                            }
                        }
                    }
                }
            };
            flow.events.push(selected);
        }
        let credit = aggregate_vector_credit_v30(&local)?;
        drop(local);
        budget.release_storage(credit)?;
        Ok(flow)
    }

    fn transfer(
        &self,
        block: usize,
        state: &mut [bool],
        validate: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionAggregateSourceErrorV30> {
        let bytes = self.allocation_cells.len();
        for operation in self.blocks[block].operations.clone() {
            budget.charge_work(2)?;
            match self.events[operation] {
                AggregateByteEventV32::Unresolved => (),
                AggregateByteEventV32::Allocate(allocation) => {
                    state[allocation] = true;
                    for cell in self.allocation_cells[allocation].clone() {
                        budget.charge_work(1)?;
                        state[bytes + cell] = false;
                    }
                }
                AggregateByteEventV32::Project(allocation) => {
                    if validate && !state[allocation] {
                        return Err(aggregate_byte_error_v32(
                            "private projection precedes dynamic allocation",
                        ));
                    }
                }
                AggregateByteEventV32::Read(leaf) | AggregateByteEventV32::Write(leaf) => {
                    let row = &self.leaves[leaf];
                    if validate && !state[row.allocation] {
                        return Err(aggregate_byte_error_v32(
                            "private access precedes dynamic allocation",
                        ));
                    }
                    for cell in row.cells.clone() {
                        budget.charge_work(1)?;
                        if matches!(self.events[operation], AggregateByteEventV32::Write(_)) {
                            state[bytes + cell] = true;
                        } else if validate && !state[bytes + cell] {
                            return Err(aggregate_byte_error_v32(
                                "private read includes uninitialized bytes",
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn check(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionAggregateSourceErrorV30> {
        let width = argument_sum_v1(&[
            self.allocation_cells.len(),
            self.allocation_cells.last().map_or(0, |cells| cells.end),
        ])?;
        let count = argument_product_v1(self.blocks.len(), width)?;
        let mut states = aggregate_byte_vec_v32(count, budget)?;
        let mut scratch = aggregate_byte_vec_v32(width, budget)?;
        let mut reached = aggregate_byte_vec_v32(self.blocks.len(), budget)?;
        let mut queue = AggregateByteQueueV32::new_retaining_scratch_v1(self.blocks.len(), budget)
            .map_err(aggregate_byte_queue_error_v32)?;
        budget.charge_work(argument_sum_v1(&[count, width, self.blocks.len()])?)?;
        states.resize(count, true);
        scratch.resize(width, false);
        reached.resize(self.blocks.len(), false);
        for (block, row) in self.blocks.iter().enumerate() {
            budget.charge_work(1)?;
            if row.entry {
                budget.charge_work(width)?;
                states[block * width..(block + 1) * width].fill(false);
                reached[block] = true;
                queue
                    .push(block, budget)
                    .map_err(aggregate_byte_queue_error_v32)?;
            }
        }
        // First reach copies the concrete predecessor's must-state; subsequent
        // edges intersect it. Bits only descend thereafter, so loop convergence
        // is bounded by the existing queue and charged lattice-cell work.
        while let Some(block) = queue.pop(budget).map_err(aggregate_byte_queue_error_v32)? {
            budget.charge_work(width)?;
            scratch.copy_from_slice(&states[block * width..(block + 1) * width]);
            self.transfer(block, &mut scratch, false, budget)?;
            for edge in self.blocks[block].edges.clone() {
                budget.charge_work(2)?;
                let target = self.edges[edge];
                let mut changed = !reached[target];
                for cell in 0..width {
                    budget.charge_work(2)?;
                    let at = target * width + cell;
                    let next = if reached[target] {
                        states[at] && scratch[cell]
                    } else {
                        scratch[cell]
                    };
                    changed |= next != states[at];
                    states[at] = next;
                }
                reached[target] = true;
                if changed {
                    queue
                        .push(target, budget)
                        .map_err(aggregate_byte_queue_error_v32)?;
                }
            }
        }
        for (block, visited) in reached.iter().enumerate() {
            budget.charge_work(1)?;
            if *visited {
                budget.charge_work(width)?;
                scratch.copy_from_slice(&states[block * width..(block + 1) * width]);
                self.transfer(block, &mut scratch, true, budget)?;
            }
        }
        Ok(())
    }
}

fn aggregate_byte_function_allocations_v32<'a>(
    allocations: &'a [AggregateMemoryAllocationBindingV31],
    cursor: &mut usize,
    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<&'a [AggregateMemoryAllocationBindingV31], ProductionAggregateSourceErrorV30> {
    budget.charge_work(2)?;
    let start = *cursor;
    while let Some(row) = allocations.get(*cursor) {
        budget.charge_work(2)?;
        if row.actual.block.function.0 < function.0 {
            return Err(aggregate_byte_error_v32(
                "private byte allocation function order differs",
            ));
        }
        if row.actual.block.function != function {
            break;
        }
        *cursor += 1;
    }
    Ok(&allocations[start..*cursor])
}

fn aggregate_byte_headers_v32() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a, 'b> = (
        AggregateBytePlanV32,
        Result<AggregateBytePlanV32, ProductionAggregateSourceErrorV30>,
        AggregateByteFlowV32,
        Result<AggregateByteFlowV32, ProductionAggregateSourceErrorV30>,
        AggregateByteQueueV32,
        Result<AggregateByteQueueV32, fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1>,
        AggregateByteInventoryV32<'a>,
        fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
        Result<
            (
                AggregateByteInventoryV32<'a>,
                fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
            ),
            fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
        >,
        [Vec<bool>; 4],
        Vec<usize>,
        [std::ops::Range<usize>; 8],
        [usize; 48],
        [bool; 8],
        [AggregateByteBoundaryV32; 3],
        [AggregateByteEventV32; 3],
        [AggregateOperationV30; 3],
        [u64; 4],
        AggregateMemoryLeafV31,
        Result<u64, ProductionAggregateSourceErrorV30>,
        Option<u32>,
        [&'a fe2o3_kernel_ir::StorageLayoutV1; 3],
        &'a fe2o3_kernel_ir::Operation,
        &'a fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'a>,
        &'a [AggregateMemoryAllocationBindingV31],
        Result<&'a [AggregateMemoryAllocationBindingV31], ProductionAggregateSourceErrorV30>,
        [&'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18; 3],
        &'a ProductionAggregateMemoryChainV31,
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
        &'a mut ArgumentBudgetV1<'b>,
        [Result<(), ProductionAggregateSourceErrorV30>; 4],
    );
    argument_sum_v1(&[
        size_of::<Frame<'_, '_>>(),
        std::mem::align_of::<Frame<'_, '_>>(),
    ])
}

fn aggregate_memory_check_chain_v32(
    memory: &ProductionAggregateMemoryChainV31,
    source: &ProductionSourceOwnedViewV18<'_>,
    chain: &fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionAggregateSourceErrorV30> {
    let floor = budget.storage();
    scoped_source_attempt_v29(source.cleanup, budget, floor, |budget| {
        let scratch_floor = budget.storage();
        let result = (|| {
            budget.reserve_storage(aggregate_byte_headers_v32()?)?;
            source.check_query_v18(budget)?;
            let original = source.canonical(budget)?;
            let plan = AggregateBytePlanV32::build(memory, original, budget)?;
            for endpoint in 0..memory.endpoint_count() {
                budget.charge_work(4)?;
                let owner = if endpoint == 0 {
                    original
                } else if endpoint % 2 == 1 {
                    chain.rounds()[(endpoint - 1) / 2].scalar().owner()
                } else {
                    chain.rounds()[endpoint / 2 - 1].aggregate().output()
                };
                for leaf in &memory.leaves[memory.endpoints[endpoint].leaves.clone()] {
                    budget.charge_work(3)?;
                    let id = leaf.original.layout.0 as usize;
                    if owner.module().storage_layouts.get(id)
                        != original.module().storage_layouts.get(id)
                    {
                        return Err(aggregate_byte_error_v32(
                            "private byte endpoint layout changed",
                        ));
                    }
                }
                let floor = budget.storage();
                scoped_source_attempt_v29(source.cleanup, budget, floor, |budget| {
                    let scratch_floor = budget.storage();
                    let checked = (|| {
                        let (inventory, receipt) =
                            AggregateByteInventoryV32::derive_v18(owner, budget)
                                .map_err(ProductionAggregateSourceErrorV30::Inventory)?;
                        budget.reserve_storage(receipt.retained_storage())?;
                        let allocations =
                            &memory.allocations[memory.endpoints[endpoint].allocations.clone()];
                        let mut cursor = 0;
                        for function in inventory.functions() {
                            let local = aggregate_byte_function_allocations_v32(
                                allocations,
                                &mut cursor,
                                function.coordinate,
                                budget,
                            )?;
                            let floor = budget.storage();
                            let result = (|| {
                                let flow = AggregateByteFlowV32::derive(
                                    &inventory, function, local, memory, endpoint, &plan, budget,
                                )?;
                                flow.check(budget)
                            })();
                            let result = source.retain_aggregate_result_v30(result);
                            if result.is_ok() {
                                budget.release_storage(
                                    budget
                                        .storage()
                                        .checked_sub(floor)
                                        .ok_or(ArgumentResourceV1::Accounting)?,
                                )?;
                            }
                            result?;
                        }
                        if cursor != allocations.len() {
                            return Err(aggregate_byte_error_v32(
                                "private byte allocation function absent",
                            ));
                        }
                        Ok(())
                    })();
                    let checked = source.retain_aggregate_result_v30(checked);
                    if checked.is_ok() {
                        budget.release_storage(
                            budget
                                .storage()
                                .checked_sub(scratch_floor)
                                .ok_or(ArgumentResourceV1::Accounting)?,
                        )?;
                    }
                    checked
                })?;
            }
            Ok(())
        })();
        let result = source.retain_aggregate_result_v30(result);
        if result.is_ok() {
            budget.release_storage(
                budget
                    .storage()
                    .checked_sub(scratch_floor)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            )?;
        }
        result
    })
}
