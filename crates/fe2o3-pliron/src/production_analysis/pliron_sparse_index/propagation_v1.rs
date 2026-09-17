fn derive_definition(
    context: &Context,
    definition: &SparseDefinitionV1,
    lattice: &[SparseIndexLatticeV1],
    definition_indices: &HashMap<Value, usize>,
    launch_extents: &[u64],
    propagation_work: &mut usize,
) -> Result<SparseIndexLatticeV1, SparseIndexFailureV1> {
    match &definition.kind {
        SparseDefinitionKindV1::EntryArgument { .. } => {
            Ok(SparseIndexLatticeV1::Known(SparseIndexFactV1::Unknown))
        }
        SparseDefinitionKindV1::Merge(inputs) => {
            merge_facts(inputs, lattice, definition_indices, propagation_work)
        }
        SparseDefinitionKindV1::Operation(operation) => {
            if let Some(guard) =
                Operation::get_op::<dialect_kernel::PublicationReadGuardOp>(*operation, context)
            {
                // Only result zero is an exact copy. The success capability and
                // the acquire value gain no sparse arithmetic facts.
                return Ok(if definition.result == guard.result(context) {
                    lookup(guard.index(context), lattice, definition_indices)
                } else {
                    known(SparseIndexFactV1::Unknown)
                });
            }
            Ok(derive_operation(
                context,
                *operation,
                lattice,
                definition_indices,
                launch_extents,
            ))
        }
    }
}

fn merge_facts(
    inputs: &[Value],
    lattice: &[SparseIndexLatticeV1],
    definition_indices: &HashMap<Value, usize>,
    propagation_work: &mut usize,
) -> Result<SparseIndexLatticeV1, SparseIndexFailureV1> {
    let mut merged = None;
    for input in inputs {
        charge_work(propagation_work, 1)?;
        let SparseIndexLatticeV1::Known(fact) = lookup(*input, lattice, definition_indices) else {
            continue;
        };
        if fact == SparseIndexFactV1::Unknown {
            return Ok(SparseIndexLatticeV1::Known(SparseIndexFactV1::Unknown));
        }
        match &merged {
            None => merged = Some(fact),
            Some(previous) if *previous == fact => {}
            Some(_) => return Ok(SparseIndexLatticeV1::Known(SparseIndexFactV1::Unknown)),
        }
    }
    Ok(merged
        .map(SparseIndexLatticeV1::Known)
        .unwrap_or(SparseIndexLatticeV1::Pending))
}

fn lookup(
    value: Value,
    lattice: &[SparseIndexLatticeV1],
    definition_indices: &HashMap<Value, usize>,
) -> SparseIndexLatticeV1 {
    definition_indices
        .get(&value)
        .map(|index| lattice[*index].clone())
        .unwrap_or(SparseIndexLatticeV1::Known(SparseIndexFactV1::Unknown))
}

// This is an execution ceiling, not a monotonicity claim: distinct overflow
// witnesses can change before a merge settles. Never erase a witness to fit it.
fn publish_sparse_fact_v1(
    current: &mut SparseIndexLatticeV1,
    next: SparseIndexLatticeV1,
    publications: &mut u8,
    work: &mut usize,
) -> Result<bool, SparseIndexFailureV1> {
    charge_work(work, 1)?;
    if *current == next {
        return Ok(false);
    }
    if *publications >= 2 {
        return Err(limit(
            "sparse fact publications",
            2,
            usize::from(*publications) + 1,
        ));
    }
    *publications += 1;
    *current = next;
    Ok(true)
}

fn charge_work(work: &mut usize, additional: usize) -> Result<(), SparseIndexFailureV1> {
    *work = work.saturating_add(additional);
    if *work > MAX_SPARSE_INDEX_WORK_UNITS_V1 {
        return Err(limit(
            "sparse propagation work",
            MAX_SPARSE_INDEX_WORK_UNITS_V1,
            *work,
        ));
    }
    Ok(())
}

fn publish_sparse_stable_root_v1(
    current: &mut SparseStableRootLatticeV1,
    next: SparseStableRootLatticeV1,
    work: &mut usize,
) -> Result<bool, SparseIndexFailureV1> {
    use SparseStableRootLatticeV1::{Known, Pending, Unknown};
    charge_work(work, 1)?;
    // Height two, enforced by the transition itself, including late poison.
    let next = match (*current, next) {
        (Unknown, _) => Unknown,
        (Known(root), Pending) => Known(root),
        (Known(previous), Known(root)) if previous != root => Unknown,
        (_, next) => next,
    };
    let changed = *current != next;
    *current = next;
    Ok(changed)
}

fn enqueue_sparse_consumers_v1(
    value: Value,
    consumers: &HashMap<Value, Vec<usize>>,
    queued: &mut [bool],
    pending: &mut VecDeque<usize>,
    work: &mut usize,
) -> Result<(), SparseIndexFailureV1> {
    if let Some(users) = consumers.get(&value) {
        for user in users {
            charge_work(work, 1)?;
            if !queued[*user] {
                queued[*user] = true;
                pending.push_back(*user);
            }
        }
    }
    Ok(())
}
