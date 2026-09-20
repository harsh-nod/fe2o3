fn effect_affine_map_is_injective(
    effect: &EffectV1,
    sparse: &SparseIndexAnalysisV1,
    launch_extents: &[u64],
    invocation_bounds: Option<&[[Option<u64>; MAX_RANKED_MEMORY_RANK]]>,
) -> bool {
    let effective_extents = effective_launch_extents(effect, launch_extents, invocation_bounds);
    affine_map_is_injective(&effect.indices, sparse, &effective_extents)
}

fn effective_launch_extents(
    effect: &EffectV1,
    launch_extents: &[u64],
    invocation_bounds: Option<&[[Option<u64>; MAX_RANKED_MEMORY_RANK]]>,
) -> Vec<u64> {
    let mut effective_extents = launch_extents.to_vec();
    if let Some(bounds) = invocation_bounds.and_then(|bounds| bounds.get(effect.location.block)) {
        for (dimension, extent) in effective_extents.iter_mut().enumerate() {
            if let Some(bound) = bounds.get(dimension).copied().flatten() {
                *extent = if *extent == 0 {
                    bound
                } else {
                    (*extent).min(bound)
                };
            }
        }
    }
    effective_extents
}

fn invocation_upper_bounds_by_block(
    context: &Context,
    function: &FuncOp,
    inventory: &crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1,
) -> Option<Vec<[Option<u64>; MAX_RANKED_MEMORY_RANK]>> {
    invocation_upper_bounds_by_block_impl(
        context,
        function,
        inventory,
        #[cfg(test)]
        None,
    )
}

#[cfg(test)]
#[derive(Default, Debug)]
struct RaceCfgStatsV1 {
    pops: usize,
    edges: usize,
    peak: usize,
    capacity: usize,
    backing: usize,
}

#[cfg(test)]
impl RaceCfgStatsV1 {
    fn queue(&mut self, queue: &VecDeque<usize>, old_capacity: usize) {
        let capacity = queue.capacity();
        self.peak = self.peak.max(queue.len());
        self.capacity = self.capacity.max(capacity);
        self.backing = self.backing.max(
            capacity
                + if capacity > old_capacity {
                    old_capacity
                } else {
                    0
                },
        );
    }
}

fn invocation_upper_bounds_by_block_impl(
    context: &Context,
    function: &FuncOp,
    inventory: &crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1,
    #[cfg(test)] mut stats: Option<&mut RaceCfgStatsV1>,
) -> Option<Vec<[Option<u64>; MAX_RANKED_MEMORY_RANK]>> {
    let blocks = inventory.blocks();
    let indices = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (*block, index))
        .collect::<HashMap<_, _>>();
    let entry = *indices.get(&function.get_entry_block(context))?;
    let empty: [Option<u64>; MAX_RANKED_MEMORY_RANK] = [None; MAX_RANKED_MEMORY_RANK];
    let mut inputs = vec![None; blocks.len()];
    inputs[entry] = Some(empty);
    let mut worklist = VecDeque::from([entry]);
    #[cfg(test)]
    if let Some(stats) = stats.as_deref_mut() {
        stats.queue(&worklist, 0);
    }
    let mut work = 0_usize;

    while let Some(block_index) = worklist.pop_front() {
        #[cfg(test)]
        if let Some(stats) = stats.as_deref_mut() {
            stats.pops += 1;
        }
        work = work.checked_add(1)?;
        if work > MAX_PLIRON_RACE_EFFECT_INSTANCES_V1 {
            return None;
        }
        let source = inputs[block_index]?;
        let terminator = blocks[block_index].deref(context).get_terminator(context)?;
        let operation = Operation::get_op_dyn(terminator, context);
        let guard = invocation_upper_bound_guard(operation.as_ref(), context);
        let raw = terminator.deref(context);
        for (successor_index, successor) in raw.successors().enumerate() {
            #[cfg(test)]
            if let Some(stats) = stats.as_deref_mut() {
                stats.edges += 1;
            }
            let target = *indices.get(&successor)?;
            if target == entry {
                continue;
            }
            let mut candidate = source;
            if successor_index == 0
                && let Some((dimension, bound)) = guard
            {
                let slot = candidate.get_mut(dimension)?;
                *slot = Some(slot.map_or(bound, |current| current.min(bound)));
            }
            let merged = match inputs[target] {
                None => candidate,
                Some(current) => std::array::from_fn(|dimension| {
                    match (current[dimension], candidate[dimension]) {
                        (Some(lhs), Some(rhs)) => Some(lhs.max(rhs)),
                        _ => None,
                    }
                }),
            };
            if inputs[target] != Some(merged) {
                inputs[target] = Some(merged);
                #[cfg(test)]
                let old_capacity = worklist.capacity();
                worklist.push_back(target);
                #[cfg(test)]
                if let Some(stats) = stats.as_deref_mut() {
                    stats.queue(&worklist, old_capacity);
                }
            }
        }
    }

    Some(
        inputs
            .into_iter()
            .map(|bounds| bounds.unwrap_or(empty))
            .collect(),
    )
}

fn invocation_upper_bound_guard(
    operation: &dyn pliron::op::Op,
    context: &Context,
) -> Option<(usize, u64)> {
    let less_than = operation
        .downcast_ref::<IndexLessThanBranchOp>()
        .map(|branch| (branch.lhs(context), branch.rhs(context)))
        .or_else(|| {
            operation
                .downcast_ref::<IndexLessThanBranchArgsOp>()
                .map(|branch| (branch.lhs(context), branch.rhs(context)))
        });
    if let Some((lhs, rhs)) = less_than {
        return Some((
            invocation_dimension(lhs, context)?,
            index_constant(rhs, context)?,
        ));
    }
    let equal = operation
        .downcast_ref::<IndexEqualBranchOp>()
        .map(|branch| (branch.lhs(context), branch.rhs(context)))
        .or_else(|| {
            operation
                .downcast_ref::<IndexEqualBranchArgsOp>()
                .map(|branch| (branch.lhs(context), branch.rhs(context)))
        })?;
    let dimension = invocation_dimension(equal.0, context)
        .filter(|_| index_constant(equal.1, context) == Some(0))
        .or_else(|| {
            invocation_dimension(equal.1, context)
                .filter(|_| index_constant(equal.0, context) == Some(0))
        })?;
    Some((dimension, 1))
}

fn invocation_dimension(value: Value, context: &Context) -> Option<usize> {
    let operation = Operation::get_op_dyn(value.defining_op()?, context);
    usize::try_from(
        operation
            .downcast_ref::<InvocationIndexOp>()?
            .dimension(context)?,
    )
    .ok()
    .filter(|dimension| *dimension < MAX_RANKED_MEMORY_RANK)
}

fn index_constant(value: Value, context: &Context) -> Option<u64> {
    let operation = Operation::get_op_dyn(value.defining_op()?, context);
    operation.downcast_ref::<IndexConstantOp>()?.value(context)
}

fn affine_facts_are_injective(facts: &[SparseAffineIndexV1], launch_extents: &[u64]) -> bool {
    if !facts
        .iter()
        .all(|affine| affine_is_total_over_launch(affine, launch_extents))
    {
        return false;
    }
    let active_dimensions = launch_extents
        .iter()
        .enumerate()
        .filter_map(|(dimension, extent)| (*extent != 1).then_some(dimension))
        .collect::<Vec<_>>();
    if active_dimensions.is_empty() {
        return true;
    }
    let matrix = facts
        .iter()
        .map(|affine| {
            active_dimensions
                .iter()
                .map(|dimension| affine.coefficients()[*dimension])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    modular_rank(matrix) == active_dimensions.len()
}

fn affine_is_total_over_launch(affine: &SparseAffineIndexV1, launch_extents: &[u64]) -> bool {
    let mut maximum = affine.constant_term();
    for (dimension, coefficient) in affine.coefficients().iter().copied().enumerate() {
        if coefficient == 0 {
            continue;
        }
        let Some(maximum_coordinate) = launch_extents
            .get(dimension)
            .copied()
            .and_then(|extent| extent.checked_sub(1))
        else {
            return false;
        };
        let Some(contribution) = coefficient.checked_mul(maximum_coordinate) else {
            return false;
        };
        let Some(next) = maximum.checked_add(contribution) else {
            return false;
        };
        maximum = next;
    }
    true
}

// Full rank modulo a prime implies full rank over the integers. A rank loss
// modulo this prime is treated as unknown and falls back to exact analysis.
fn modular_rank(mut matrix: Vec<Vec<u64>>) -> usize {
    const PRIME: u64 = (1_u64 << 61) - 1;
    let row_count = matrix.len();
    let column_count = matrix.first().map_or(0, Vec::len);
    for row in &mut matrix {
        for value in row {
            *value %= PRIME;
        }
    }
    let mut rank = 0_usize;
    for column in 0..column_count {
        let Some(pivot) = (rank..row_count).find(|row| matrix[*row][column] != 0) else {
            continue;
        };
        matrix.swap(rank, pivot);
        let inverse = modular_power(matrix[rank][column], PRIME - 2, PRIME);
        for value in &mut matrix[rank][column..column_count] {
            *value = modular_multiply(*value, inverse, PRIME);
        }
        let pivot_row = matrix[rank].clone();
        for (row_index, row) in matrix.iter_mut().enumerate() {
            if row_index == rank || row[column] == 0 {
                continue;
            }
            let factor = row[column];
            for (value, pivot) in row[column..column_count]
                .iter_mut()
                .zip(&pivot_row[column..column_count])
            {
                let product = modular_multiply(factor, *pivot, PRIME);
                *value = (*value + PRIME - product) % PRIME;
            }
        }
        rank += 1;
        if rank == row_count {
            break;
        }
    }
    rank
}

fn modular_power(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    let mut result = 1_u64;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = modular_multiply(result, base, modulus);
        }
        base = modular_multiply(base, base, modulus);
        exponent >>= 1;
    }
    result
}

fn modular_multiply(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    ((u128::from(lhs) * u128::from(rhs)) % u128::from(modulus)) as u64
}

fn decode_invocation(mut linear: u64, extents: &[u64]) -> Vec<u64> {
    let mut invocation = Vec::with_capacity(extents.len());
    for extent in extents {
        invocation.push(linear % extent);
        linear /= extent;
    }
    invocation
}

fn sparse_failure(failure: SparseIndexFailureV1) -> String {
    match failure {
        SparseIndexFailureV1::ResourceLimit {
            resource,
            limit,
            actual,
        } => format!("{resource} count {actual} exceeds {limit}"),
        SparseIndexFailureV1::InconsistentLaunchExtent {
            dimension,
            first,
            second,
        } => format!(
            "invocation dimension {dimension} has inconsistent launch extents {first} and {second}"
        ),
        SparseIndexFailureV1::MalformedControlFlow { detail } => detail.to_owned(),
    }
}

fn one(finding: RankedRaceFindingV1) -> RankedRaceReportV1 {
    RankedRaceReportV1 {
        findings: vec![finding],
    }
}

fn clean() -> RankedRaceReportV1 {
    RankedRaceReportV1 {
        findings: Vec::new(),
    }
}
