use super::*;
use primitives::{add, limit, need};

pub(super) fn operation(reader: &mut Reader<'_, '_, '_>) -> R<Option<Op>> {
    let before = reader.shape.expressions;
    let tag = reader.u16()?;
    reader.zero()?;
    let result = match tag {
        1 => {
            let grid_identity = reader.u64()?;
            let global_extents = reader.array()?;
            let workgroup_extents = reader.array()?;
            let subgroup_size = reader.u64()?;
            let full_physical_workgroups = reader.boolean()?;
            reader.build(|| {
                Ok(Op::ExecutionLayout {
                    grid_identity,
                    global_extents,
                    workgroup_extents,
                    subgroup_size,
                    full_physical_workgroups,
                })
            })?
        }
        2 => {
            let result = Id::new(reader.u32()?);
            let element_width = reader.u32()?;
            let writable = reader.boolean()?;
            let shape = reader.widths(MAX_RANKED_MEMORY_RANK)?;
            let dynamic_extents = reader.values(MAX_RANKED_MEMORY_RANK)?;
            let allocation_origin = reader.u64()?;
            let noalias_class = reader.u64()?;
            reader.build(|| {
                Ok(Op::View {
                    result,
                    element_width,
                    writable,
                    shape,
                    dynamic_extents,
                    allocation_origin,
                    noalias_class,
                })
            })?
        }
        3 => {
            let result = Id::new(reader.u32()?);
            let element_width = reader.u32()?;
            let writable = reader.boolean()?;
            let shape = reader.widths(MAX_RANKED_MEMORY_RANK)?;
            let dynamic_extents = reader.values(MAX_RANKED_MEMORY_RANK)?;
            let memory_space = reader.memory_space()?;
            let allocation_origin = reader.u64()?;
            let noalias_class = reader.u64()?;
            reader.build(|| {
                Ok(Op::ViewInSpace {
                    result,
                    element_width,
                    writable,
                    shape,
                    dynamic_extents,
                    memory_space,
                    allocation_origin,
                    noalias_class,
                })
            })?
        }
        4 => {
            let result = Id::new(reader.u32()?);
            let view = reader.value()?;
            let buffers = reader.u32()?;
            let prefetch_distance = reader.u32()?;
            reader.build(|| {
                Ok(Op::PipelineCreate {
                    result,
                    view,
                    buffers,
                    prefetch_distance,
                })
            })?
        }
        5 => {
            let pipeline = reader.value()?;
            let epoch = reader.value()?;
            let slot = reader.value()?;
            let kind = reader.pipeline_event()?;
            reader.build(|| {
                Ok(Op::PipelineEvent {
                    pipeline,
                    epoch,
                    slot,
                    kind,
                })
            })?
        }
        6 => {
            let result = Id::new(reader.u32()?);
            let value = reader.u64()?;
            reader.build(|| Ok(Op::IndexConstant { result, value }))?
        }
        7 => {
            let result = Id::new(reader.u32()?);
            let source = reader.value()?;
            let bit_width = reader.u16()?;
            reader.build(|| {
                Ok(Op::IndexUnsignedCast {
                    result,
                    source,
                    bit_width,
                })
            })?
        }
        8 => {
            let result = Id::new(reader.u32()?);
            reader.build(|| Ok(Op::IndexUnknown { result }))?
        }
        9 => {
            let result = Id::new(reader.u32()?);
            let dimension = reader.u32()?;
            let launch_extent = reader.u64()?;
            reader.build(|| {
                Ok(Op::InvocationIndex {
                    result,
                    dimension,
                    launch_extent,
                })
            })?
        }
        10 => {
            let result = Id::new(reader.u32()?);
            let kind = reader.index_kind()?;
            let lhs = reader.value()?;
            let rhs = reader.value()?;
            reader.build(|| {
                Ok(Op::IndexBinary {
                    result,
                    kind,
                    lhs,
                    rhs,
                })
            })?
        }
        11 => {
            let result = Id::new(reader.u32()?);
            let dependencies = reader.values(MAX_DETERMINISTIC_JOIN_INPUTS_V1)?;
            reader.build(|| {
                Ok(Op::DeterministicJoin {
                    result,
                    dependencies,
                })
            })?
        }
        12 => {
            let result = Id::new(reader.u32()?);
            let invocation = reader.value()?;
            let component = reader.value()?;
            let rows = reader.value()?;
            let columns = reader.value()?;
            let row_stride = reader.value()?;
            let lanes_per_tile = reader.u64()?;
            let tile_rows = reader.u64()?;
            let tile_columns = reader.u64()?;
            let elements_per_lane = reader.u64()?;
            reader.build(|| {
                Ok(Op::CheckedTiledIndex2D {
                    result,
                    invocation,
                    component,
                    rows,
                    columns,
                    row_stride,
                    lanes_per_tile,
                    tile_rows,
                    tile_columns,
                    elements_per_lane,
                })
            })?
        }
        13 => {
            let result = Id::new(reader.u32()?);
            let invocation = reader.value()?;
            let component = reader.value()?;
            let rows = reader.value()?;
            let columns = reader.value()?;
            let row_stride = reader.value()?;
            let lanes_per_row = reader.u64()?;
            let elements_per_lane = reader.u64()?;
            reader.build(|| {
                Ok(Op::CheckedRowStripedIndex2D {
                    result,
                    invocation,
                    component,
                    rows,
                    columns,
                    row_stride,
                    lanes_per_row,
                    elements_per_lane,
                })
            })?
        }
        14 => {
            let result = Id::new(reader.u32()?);
            let success = Id::new(reader.u32()?);
            let invocation = reader.value()?;
            let component = reader.value()?;
            let rows = reader.value()?;
            let columns = reader.value()?;
            let row_stride = reader.value()?;
            let physical_extent = reader.value()?;
            let lanes_per_tile = reader.u64()?;
            let tile_rows = reader.u64()?;
            let tile_columns = reader.u64()?;
            let elements_per_lane = reader.u64()?;
            reader.build(|| {
                Ok(Op::PredicatedCheckedTiledIndex2D {
                    result,
                    success,
                    invocation,
                    component,
                    rows,
                    columns,
                    row_stride,
                    physical_extent,
                    lanes_per_tile,
                    tile_rows,
                    tile_columns,
                    elements_per_lane,
                })
            })?
        }
        15 => {
            let result = Id::new(reader.u32()?);
            let success = Id::new(reader.u32()?);
            let invocation = reader.value()?;
            let component = reader.value()?;
            let rows = reader.value()?;
            let columns = reader.value()?;
            let row_stride = reader.value()?;
            let physical_extent = reader.value()?;
            let lanes_per_row = reader.u64()?;
            let elements_per_lane = reader.u64()?;
            reader.build(|| {
                Ok(Op::PredicatedCheckedRowStripedIndex2D {
                    result,
                    success,
                    invocation,
                    component,
                    rows,
                    columns,
                    row_stride,
                    physical_extent,
                    lanes_per_row,
                    elements_per_lane,
                })
            })?
        }
        16 => {
            let result = Id::new(reader.u32()?);
            let view = reader.value()?;
            let dimension = reader.u32()?;
            reader.build(|| {
                Ok(Op::Dimension {
                    result,
                    view,
                    dimension,
                })
            })?
        }
        17 => {
            let kind = reader.access()?;
            let view = reader.value()?;
            let indices = reader.values(MAX_RANKED_MEMORY_RANK)?;
            reader.build(|| {
                Ok(Op::Access {
                    kind,
                    view,
                    indices,
                })
            })?
        }
        18 => {
            let kind = reader.access()?;
            let view = reader.value()?;
            let index = reader.value()?;
            let success = reader.value()?;
            reader.build(|| {
                Ok(Op::PredicatedAccess {
                    kind,
                    view,
                    index,
                    success,
                })
            })?
        }
        19 => {
            let kind = reader.access()?;
            let view = reader.value()?;
            let indices = reader.values(MAX_RANKED_MEMORY_RANK)?;
            let value = reader.value()?;
            reader.build(|| {
                Ok(Op::ValueAccess {
                    kind,
                    view,
                    indices,
                    value,
                })
            })?
        }
        20 => {
            let kind = reader.access()?;
            let ordering = reader.ordering()?;
            let scope = reader.atomic_scope()?;
            let view = reader.value()?;
            let indices = reader.values(MAX_RANKED_MEMORY_RANK)?;
            reader.build(|| {
                Ok(Op::AtomicAccess {
                    kind,
                    ordering,
                    scope,
                    view,
                    indices,
                })
            })?
        }
        21 => {
            let kind = reader.access()?;
            let ordering = reader.ordering()?;
            let scope = reader.atomic_scope()?;
            let view = reader.value()?;
            let indices = reader.values(MAX_RANKED_MEMORY_RANK)?;
            let value = reader.value()?;
            reader.build(|| {
                Ok(Op::AtomicValueAccess {
                    kind,
                    ordering,
                    scope,
                    view,
                    indices,
                    value,
                })
            })?
        }
        22 => {
            let view = reader.value()?;
            let coverage = reader.coverage()?;
            let partition = reader.partition()?;
            reader.build(|| {
                Ok(Op::OwnershipContract {
                    view,
                    coverage,
                    partition,
                })
            })?
        }
        23 => {
            let kind = reader.access()?;
            let memory_space = reader.memory_space()?;
            let allocation_origin = reader.u64()?;
            let noalias_class = reader.u64()?;
            reader.build(|| {
                Ok(Op::AllocationEffect {
                    kind,
                    memory_space,
                    allocation_origin,
                    noalias_class,
                })
            })?
        }
        24 => {
            let execution_scope = reader.hierarchy()?;
            let memory_scope = reader.memory_scope()?;
            let address_space = reader.address_space()?;
            let order = reader.memory_order()?;
            reader.build(|| {
                Ok(Op::Barrier {
                    execution_scope,
                    memory_scope,
                    address_space,
                    order,
                })
            })?
        }
        25 => {
            let memory_scope = reader.memory_scope()?;
            let address_space = reader.address_space()?;
            let order = reader.memory_order()?;
            reader.build(|| {
                Ok(Op::Fence {
                    memory_scope,
                    address_space,
                    order,
                })
            })?
        }
        26 => {
            let contract = reader.tensor()?;
            let convergence = reader.convergence()?;
            let active_lanes = reader.u32()?;
            let binding = reader.cooperative()?;
            reader.build(|| {
                Ok(Op::TensorLayout {
                    contract: need(contract)?,
                    convergence,
                    active_lanes,
                    binding,
                })
            })?
        }
        27 => {
            let result = Id::new(reader.u32()?);
            let tensor_result_root = reader.digest()?;
            let component = reader.u16()?;
            let scalar = reader.scalar()?;
            let numerical_contract = reader.numerical()?;
            reader.build(|| {
                Ok(Op::TensorResultComponent {
                    result,
                    tensor_result_root,
                    component,
                    scalar,
                    numerical_contract,
                })
            })?
        }
        28 => {
            let result = Id::new(reader.u32()?);
            let symbol = reader.u32()?;
            reader.build(|| Ok(Op::SemanticSymbol { result, symbol }))?
        }
        29 => {
            let result = Id::new(reader.u32()?);
            let value = reader.u64()?;
            reader.build(|| Ok(Op::SemanticConstant { result, value }))?
        }
        30 => {
            let result = Id::new(reader.u32()?);
            let kind = reader.semantic_binary()?;
            let lhs = reader.value()?;
            let rhs = reader.value()?;
            reader.build(|| {
                Ok(Op::SemanticBinary {
                    result,
                    kind,
                    lhs,
                    rhs,
                })
            })?
        }
        31 => {
            let result = Id::new(reader.u32()?);
            let expression = reader.expression()?;
            let numerical_contract = reader.numerical()?;
            reader.build(|| {
                Ok(Op::SemanticExpression {
                    result,
                    expression: need(expression)?,
                    numerical_contract,
                })
            })?
        }
        32 => {
            let contract = reader.collective()?;
            let view = reader.value()?;
            let actual = reader.value()?;
            let expected = reader.value()?;
            let witness0 = reader.value()?;
            let witness1 = reader.value()?;
            reader.build(|| {
                Ok(Op::CollectiveSemantics {
                    contract: need(contract)?,
                    view,
                    actual,
                    expected,
                    witness0,
                    witness1,
                })
            })?
        }
        33 => {
            let actual = reader.value()?;
            let expected = reader.value()?;
            reader.build(|| Ok(Op::RequireEquivalent { actual, expected }))?
        }
        34 => {
            let actual = reader.value()?;
            let expected = reader.value()?;
            let proof = reader.proof()?;
            reader.build(|| {
                Ok(Op::RequireAuthenticatedReferenceEquivalent {
                    actual,
                    expected,
                    proof: need(proof)?,
                })
            })?
        }
        35 => {
            let actual = reader.value()?;
            let expected = reader.value()?;
            let subjects = reader.subjects()?;
            reader.build(|| {
                Ok(Op::RequestAuthenticatedReferenceEquivalent {
                    actual,
                    expected,
                    subjects: need(subjects)?,
                })
            })?
        }
        36 => {
            let contract = reader.effect()?;
            let proof = reader.proof()?;
            reader.build(|| {
                Ok(Op::RequireEffectRefinement {
                    contract: need(contract)?,
                    proof: need(proof)?,
                })
            })?
        }
        37 => {
            let contract = reader.effect()?;
            let subjects = reader.subjects()?;
            reader.build(|| {
                Ok(Op::RequestEffectRefinement {
                    contract: need(contract)?,
                    subjects: need(subjects)?,
                })
            })?
        }
        38 => {
            let contract = reader.numerical_refinement()?;
            let proof = reader.proof()?;
            reader.build(|| {
                Ok(Op::RequireNumericalRefinement {
                    contract: need(contract)?,
                    proof: need(proof)?,
                })
            })?
        }
        39 => {
            let contract = reader.numerical_refinement()?;
            let subjects = reader.subjects()?;
            reader.build(|| {
                Ok(Op::RequestNumericalRefinement {
                    contract: need(contract)?,
                    subjects: need(subjects)?,
                })
            })?
        }
        40 => {
            let contract = reader.tensor_refinement()?;
            let proof = reader.proof()?;
            reader.build(|| {
                Ok(Op::RequireTensorRefinement {
                    contract: need(contract)?,
                    proof: need(proof)?,
                })
            })?
        }
        41 => {
            let contract = reader.tensor_refinement()?;
            let subjects = reader.subjects()?;
            reader.build(|| {
                Ok(Op::RequestTensorRefinement {
                    contract: need(contract)?,
                    subjects: need(subjects)?,
                })
            })?
        }
        tag => {
            return Err(E::Tag {
                field: "operation",
                tag,
            });
        }
    };
    reader
        .shape
        .operation(tag, reader.shape.expressions - before)?;
    Ok(result)
}
pub(super) fn terminator(reader: &mut Reader<'_, '_, '_>) -> R<Option<Term>> {
    let tag = reader.u16()?;
    reader.zero()?;
    let result = match tag {
        1 => {
            let lhs = reader.value()?;
            let rhs = reader.value()?;
            let true_block = reader.u32()?;
            let false_block = reader.u32()?;
            reader.build(|| {
                Ok(Term::IndexLessThan {
                    lhs,
                    rhs,
                    true_block,
                    false_block,
                })
            })?
        }
        2 => {
            let lhs = reader.value()?;
            let rhs = reader.value()?;
            let true_arguments = reader.values(HARD_MAX_PRODUCTION_RANKED_ARGUMENTS)?;
            let false_arguments = reader.values(HARD_MAX_PRODUCTION_RANKED_ARGUMENTS)?;
            let true_block = reader.u32()?;
            let false_block = reader.u32()?;
            reader.build(|| {
                Ok(Term::IndexLessThanArgs {
                    lhs,
                    rhs,
                    true_arguments,
                    false_arguments,
                    true_block,
                    false_block,
                })
            })?
        }
        3 => {
            let lhs = reader.value()?;
            let rhs = reader.value()?;
            let true_block = reader.u32()?;
            let false_block = reader.u32()?;
            reader.build(|| {
                Ok(Term::IndexEqual {
                    lhs,
                    rhs,
                    true_block,
                    false_block,
                })
            })?
        }
        4 => {
            let lhs = reader.value()?;
            let rhs = reader.value()?;
            let true_arguments = reader.values(HARD_MAX_PRODUCTION_RANKED_ARGUMENTS)?;
            let false_arguments = reader.values(HARD_MAX_PRODUCTION_RANKED_ARGUMENTS)?;
            let true_block = reader.u32()?;
            let false_block = reader.u32()?;
            reader.build(|| {
                Ok(Term::IndexEqualArgs {
                    lhs,
                    rhs,
                    true_arguments,
                    false_arguments,
                    true_block,
                    false_block,
                })
            })?
        }
        5 => {
            let control_dependencies = reader.values(MAX_RANKED_RECIPE_BYTES_V1 / 6)?;
            let first_block = reader.u32()?;
            let second_block = reader.u32()?;
            reader.build(|| {
                Ok(Term::AnalysisSplit {
                    control_dependencies,
                    first_block,
                    second_block,
                })
            })?
        }
        6 => {
            let control_dependencies = reader.values(MAX_RANKED_RECIPE_BYTES_V1 / 6)?;
            let first_arguments = reader.values(HARD_MAX_PRODUCTION_RANKED_ARGUMENTS)?;
            let second_arguments = reader.values(HARD_MAX_PRODUCTION_RANKED_ARGUMENTS)?;
            let first_block = reader.u32()?;
            let second_block = reader.u32()?;
            reader.build(|| {
                Ok(Term::AnalysisSplitArgs {
                    control_dependencies,
                    first_arguments,
                    second_arguments,
                    first_block,
                    second_block,
                })
            })?
        }
        7 => {
            let target = reader.u32()?;
            reader.build(|| Ok(Term::Branch { target }))?
        }
        8 => {
            let arguments = reader.values(HARD_MAX_PRODUCTION_RANKED_ARGUMENTS)?;
            let target = reader.u32()?;
            reader.build(|| Ok(Term::BranchArgs { arguments, target }))?
        }
        9 => {
            let value = reader.value()?;
            let step = reader.value()?;
            let target = reader.u32()?;
            reader.build(|| {
                Ok(Term::BranchArgsAdd {
                    value,
                    step,
                    target,
                })
            })?
        }
        10 => {
            let arguments = reader.values(HARD_MAX_PRODUCTION_RANKED_ARGUMENTS)?;
            let add_argument = reader.u32()?;
            let step = reader.value()?;
            let target = reader.u32()?;
            reader.build(|| {
                Ok(Term::BranchArgsAddAt {
                    arguments,
                    add_argument,
                    step,
                    target,
                })
            })?
        }
        11 => reader.build(|| Ok(Term::Return))?,
        12 => reader.build(|| Ok(Term::Trap))?,
        tag => {
            return Err(E::Tag {
                field: "terminator",
                tag,
            });
        }
    };
    reader.shape.terminator(tag)?;
    Ok(result)
}

pub(super) type Parsed<'a> = (&'a str, usize, Vec<Block>, Shape, usize);

fn parse<'wire>(
    bytes: &'wire [u8],
    materialize: bool,
    budget: &mut Budget<'_>,
) -> R<Parsed<'wire>> {
    limit("recipe bytes", bytes.len(), MAX_RANKED_RECIPE_BYTES_V1)?;
    if bytes.len() < HEADER {
        return Err(E::Length);
    }
    let mut reader = Reader::new(bytes, materialize, budget);
    if reader.take(8)? != RANKED_RECIPE_MAGIC_V1 {
        return Err(E::Header);
    }
    if reader.u16()? != 1 {
        return Err(E::Header);
    }
    reader.zero()?;
    if reader.u32()? != HEADER as u32 {
        return Err(E::Header);
    }
    if reader.u64()? != bytes.len() as u64 {
        return Err(E::Length);
    }
    let blocks = reader.count(MAX_RANKED_BOUNDS_BLOCKS, 12, "blocks")?;
    let operations = reader.count(MAX_RANKED_BOUNDS_OPERATIONS, 4, "operations")?;
    let expressions = reader.count(MAX_RANKED_BOUNDS_OPERATIONS, 2, "expressions")?;
    let arguments = reader.count(HARD_MAX_PRODUCTION_RANKED_ARGUMENTS, 0, "arguments")?;
    let name_length = reader.count(crate::HARD_MAX_NAME_BYTES, 1, "name")?;
    if reader.u32()? != 0 {
        return Err(E::Reserved);
    }
    let name = std::str::from_utf8(reader.take(name_length)?).map_err(|_| E::Utf8)?;
    let mut output = reader.vector(blocks)?;
    for _ in 0..blocks {
        let count = reader.count(HARD_MAX_PRODUCTION_RANKED_ARGUMENTS, 0, "block arguments")?;
        reader.shape.block(count)?;
        let op_count = reader.count(MAX_RANKED_BOUNDS_OPERATIONS, 4, "block operations")?;
        let mut values = reader.vector(op_count)?;
        for _ in 0..op_count {
            if let Some(value) = operation(&mut reader)? {
                values.push(value);
            }
        }
        let terminator = terminator(&mut reader)?;
        if materialize {
            output.push(Block::with_index_arguments(
                count as u32,
                values,
                need(terminator)?,
            ));
        }
    }
    if reader.position != bytes.len() {
        return Err(E::Length);
    }
    reader.shape.finish()?;
    if reader.shape.blocks != blocks
        || reader.shape.operations != operations
        || reader.shape.expressions != expressions
    {
        return Err(E::Header);
    }
    Ok((name, arguments, output, reader.shape, reader.heap))
}

pub(super) fn scan(bytes: &[u8], budget: &mut Budget<'_>) -> R<Shape> {
    let (_, _, blocks, shape, heap) = parse(bytes, false, budget)?;
    if heap != 0 || !blocks.is_empty() {
        return Err(Resource::Accounting.into());
    }
    Ok(shape)
}

pub(super) fn materialize(bytes: &[u8], budget: &mut Budget<'_>) -> R<(Kernel, usize)> {
    let (name, arguments, blocks, shape, input_heap) = parse(bytes, true, budget)?;
    // Unchanged legacy constructor owns its bounded transforms/validation. Both
    // its consumed input and provisional returned tree remain paid at the join.
    let provisional = add(input_heap, name.len())?;
    let mut visits = shape.census_visits()?;
    budget.charge_work(visits)?;
    let census_frames = if shape.expressions == 0 {
        0
    } else {
        primitives::product(
            crate::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2,
            size_of::<(&Expr, usize, usize)>(),
        )?
    };
    budget.reserve_storage(census_frames)?;
    budget.reserve_storage(provisional)?;
    let kernel = Kernel::new(name, arguments, blocks).map_err(E::Constructor)?;
    let retained = operations::retained_heap(&kernel, &mut visits)?;
    if visits != 0 {
        return Err(Resource::Accounting.into());
    }
    budget.release_storage(census_frames)?;
    if retained > provisional {
        budget.reserve_storage(retained - provisional)?;
    } else {
        budget.release_storage(provisional - retained)?;
    }
    budget.release_storage(input_heap)?;
    Ok((kernel, retained))
}
