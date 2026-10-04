// Copy identity only: no arithmetic, cast, subgroup, or general uniformity rule.
// Values and the analysis owner are context-qualified handles in pinned pliron.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SparseStableRootV1 {
    EntryArgument(Value),
    Constant(u64),
}

#[derive(Clone, Debug)]
struct SparseValueFactsV1 {
    numeric: SparseIndexFactV1,
    stable: Option<SparseStableRootV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SparseStableRootLatticeV1 {
    Pending,
    Known(SparseStableRootV1),
    Unknown,
}

fn initialize_sparse_stable_roots_v1(
    context: &Context,
    entry: Ptr<BasicBlock>,
    definitions: &[SparseDefinitionV1],
    work: &mut usize,
) -> Result<Vec<SparseStableRootLatticeV1>, SparseIndexFailureV1> {
    // Prepay allocation initialization, export, and drop of eight-cell roots,
    // plus the vector header and retained owner. No new graph is collected.
    charge_work(
        work,
        definitions.len().saturating_mul(32).saturating_add(16),
    )?;
    let mut roots = Vec::with_capacity(definitions.len());
    for definition in definitions {
        roots.push(seed_sparse_stable_root_v1(
            context, entry, definition, work,
        )?);
    }
    Ok(roots)
}

fn seed_sparse_stable_root_v1(
    context: &Context,
    entry: Ptr<BasicBlock>,
    definition: &SparseDefinitionV1,
    work: &mut usize,
) -> Result<SparseStableRootLatticeV1, SparseIndexFailureV1> {
    use SparseStableRootLatticeV1::{Known, Pending, Unknown};
    use dialect_kernel::IndexType;
    charge_work(work, 16)?;
    match &definition.kind {
        SparseDefinitionKindV1::Merge(_) => Ok(Pending),
        SparseDefinitionKindV1::EntryArgument { ordinal } => {
            if definition.result.defining_block() != Some(entry) {
                return Err(malformed(
                    "stable root is not an argument of the function entry",
                ));
            }
            let block = entry
                .try_deref(context)
                .map_err(|_| malformed("stable root entry owner is unavailable"))?;
            if *ordinal >= block.get_num_arguments()
                || block.get_argument(*ordinal) != definition.result
            {
                return Err(malformed(
                    "stable root entry argument is absent from its recorded ordinal",
                ));
            }
            // Exact argument identity is type-independent. Consumers still
            // require their own verified operand type and semantic contract.
            Ok(Known(SparseStableRootV1::EntryArgument(definition.result)))
        }
        SparseDefinitionKindV1::Operation(operation) => {
            if definition.result.defining_op() != Some(*operation) {
                return Err(malformed(
                    "stable root result has a different operation owner",
                ));
            }
            let raw = operation
                .try_deref(context)
                .map_err(|_| malformed("stable root operation owner is unavailable"))?;
            let block = raw
                .get_parent_block()
                .ok_or_else(|| malformed("stable root operation is detached"))?;
            let parent = block
                .try_deref(context)
                .map_err(|_| malformed("stable root block owner is unavailable"))?;
            let entry = entry
                .try_deref(context)
                .map_err(|_| malformed("stable root entry owner is unavailable"))?;
            if parent.get_parent_region().is_none()
                || parent.get_parent_region() != entry.get_parent_region()
            {
                return Err(malformed(
                    "stable root operation belongs to another function",
                ));
            }
            let dynamic = Operation::get_op_dyn(*operation, context);
            let Some(constant) = dynamic.downcast_ref::<IndexConstantOp>() else {
                return Ok(Unknown);
            };
            charge_work(work, 8)?;
            if raw.get_num_results() != 1
                || raw.get_result(0) != definition.result
                || !definition
                    .result
                    .get_type(context)
                    .deref(context)
                    .is::<IndexType>()
            {
                return Err(malformed("stable literal result does not match its owner"));
            }
            Ok(constant
                .value(context)
                .map(|value| Known(SparseStableRootV1::Constant(value)))
                .unwrap_or(Unknown))
        }
    }
}

fn derive_sparse_stable_root_v1(
    definition: &SparseDefinitionV1,
    current: SparseStableRootLatticeV1,
    roots: &[SparseStableRootLatticeV1],
    definition_indices: &HashMap<Value, usize>,
    work: &mut usize,
) -> Result<SparseStableRootLatticeV1, SparseIndexFailureV1> {
    use SparseStableRootLatticeV1::{Known, Pending, Unknown};
    charge_work(work, 1)?;
    let SparseDefinitionKindV1::Merge(inputs) = &definition.kind else {
        return Ok(current);
    };
    let mut merged = Pending;
    for input in inputs {
        charge_work(work, 1)?;
        let next = definition_indices
            .get(input)
            .map(|index| roots[*index])
            .unwrap_or(Unknown);
        match (merged, next) {
            (_, Unknown) => return Ok(Unknown),
            (_, Pending) => {}
            (Pending, Known(root)) => merged = Known(root),
            (Known(previous), Known(root)) if previous == root => {}
            (Known(_), Known(_)) => return Ok(Unknown),
            (Unknown, _) => return Ok(Unknown),
        }
    }
    Ok(merged)
}

#[cfg(test)]
include!("stable_roots_v1_tests.rs");
