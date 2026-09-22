use super::*;
use primitives::{add, product};

pub(super) fn body(writer: &mut Writer<'_, '_>, kernel: &Kernel) -> R<()> {
    writer.bytes(kernel.function_name.as_bytes())?;
    for block in &kernel.blocks {
        writer.shape.block(block.index_argument_count as usize)?;
        writer.u32(block.index_argument_count)?;
        writer.count(
            block.operations.len(),
            MAX_RANKED_BOUNDS_OPERATIONS,
            "block operations",
        )?;
        for value in &block.operations {
            operation(writer, value)?;
        }
        terminator(writer, &block.terminator)?;
    }
    Ok(())
}
pub(super) fn operation(writer: &mut Writer<'_, '_>, operation: &Op) -> R<()> {
    let before = writer.shape.expressions;
    let tag = match operation {
        Op::ExecutionLayout {
            grid_identity,
            global_extents,
            workgroup_extents,
            subgroup_size,
            full_physical_workgroups,
        } => {
            writer.u16(1)?;
            writer.u16(0)?;
            writer.u64(*grid_identity)?;
            writer.array(global_extents)?;
            writer.array(workgroup_extents)?;
            writer.u64(*subgroup_size)?;
            writer.boolean(*full_physical_workgroups)?;
            1
        }
        Op::View {
            result,
            element_width,
            writable,
            shape,
            dynamic_extents,
            allocation_origin,
            noalias_class,
        } => {
            writer.u16(2)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            writer.u32(*element_width)?;
            writer.boolean(*writable)?;
            writer.widths(shape, MAX_RANKED_MEMORY_RANK)?;
            writer.values(dynamic_extents, MAX_RANKED_MEMORY_RANK)?;
            writer.u64(*allocation_origin)?;
            writer.u64(*noalias_class)?;
            2
        }
        Op::ViewInSpace {
            result,
            element_width,
            writable,
            shape,
            dynamic_extents,
            memory_space,
            allocation_origin,
            noalias_class,
        } => {
            writer.u16(3)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            writer.u32(*element_width)?;
            writer.boolean(*writable)?;
            writer.widths(shape, MAX_RANKED_MEMORY_RANK)?;
            writer.values(dynamic_extents, MAX_RANKED_MEMORY_RANK)?;
            writer.memory_space(*memory_space)?;
            writer.u64(*allocation_origin)?;
            writer.u64(*noalias_class)?;
            3
        }
        Op::PipelineCreate {
            result,
            view,
            buffers,
            prefetch_distance,
        } => {
            writer.u16(4)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            writer.value(*view)?;
            writer.u32(*buffers)?;
            writer.u32(*prefetch_distance)?;
            4
        }
        Op::PipelineEvent {
            pipeline,
            epoch,
            slot,
            kind,
        } => {
            writer.u16(5)?;
            writer.u16(0)?;
            writer.value(*pipeline)?;
            writer.value(*epoch)?;
            writer.value(*slot)?;
            writer.pipeline_event(*kind)?;
            5
        }
        Op::IndexConstant { result, value } => {
            writer.u16(6)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            writer.u64(*value)?;
            6
        }
        Op::IndexUnsignedCast {
            result,
            source,
            bit_width,
        } => {
            writer.u16(7)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            writer.value(*source)?;
            writer.u16(*bit_width)?;
            7
        }
        Op::IndexUnknown { result } => {
            writer.u16(8)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            8
        }
        Op::InvocationIndex {
            result,
            dimension,
            launch_extent,
        } => {
            writer.u16(9)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            writer.u32(*dimension)?;
            writer.u64(*launch_extent)?;
            9
        }
        Op::IndexBinary {
            result,
            kind,
            lhs,
            rhs,
        } => {
            writer.u16(10)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            writer.index_kind(*kind)?;
            writer.value(*lhs)?;
            writer.value(*rhs)?;
            10
        }
        Op::DeterministicJoin {
            result,
            dependencies,
        } => {
            writer.u16(11)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            writer.values(dependencies, MAX_DETERMINISTIC_JOIN_INPUTS_V1)?;
            11
        }
        Op::CheckedTiledIndex2D {
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
        } => {
            writer.u16(12)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            writer.value(*invocation)?;
            writer.value(*component)?;
            writer.value(*rows)?;
            writer.value(*columns)?;
            writer.value(*row_stride)?;
            writer.u64(*lanes_per_tile)?;
            writer.u64(*tile_rows)?;
            writer.u64(*tile_columns)?;
            writer.u64(*elements_per_lane)?;
            12
        }
        Op::CheckedRowStripedIndex2D {
            result,
            invocation,
            component,
            rows,
            columns,
            row_stride,
            lanes_per_row,
            elements_per_lane,
        } => {
            writer.u16(13)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            writer.value(*invocation)?;
            writer.value(*component)?;
            writer.value(*rows)?;
            writer.value(*columns)?;
            writer.value(*row_stride)?;
            writer.u64(*lanes_per_row)?;
            writer.u64(*elements_per_lane)?;
            13
        }
        Op::PredicatedCheckedTiledIndex2D {
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
        } => {
            writer.u16(14)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            writer.u32(success.get())?;
            writer.value(*invocation)?;
            writer.value(*component)?;
            writer.value(*rows)?;
            writer.value(*columns)?;
            writer.value(*row_stride)?;
            writer.value(*physical_extent)?;
            writer.u64(*lanes_per_tile)?;
            writer.u64(*tile_rows)?;
            writer.u64(*tile_columns)?;
            writer.u64(*elements_per_lane)?;
            14
        }
        Op::PredicatedCheckedRowStripedIndex2D {
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
        } => {
            writer.u16(15)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            writer.u32(success.get())?;
            writer.value(*invocation)?;
            writer.value(*component)?;
            writer.value(*rows)?;
            writer.value(*columns)?;
            writer.value(*row_stride)?;
            writer.value(*physical_extent)?;
            writer.u64(*lanes_per_row)?;
            writer.u64(*elements_per_lane)?;
            15
        }
        Op::Dimension {
            result,
            view,
            dimension,
        } => {
            writer.u16(16)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            writer.value(*view)?;
            writer.u32(*dimension)?;
            16
        }
        Op::Access {
            kind,
            view,
            indices,
        } => {
            writer.u16(17)?;
            writer.u16(0)?;
            writer.access(*kind)?;
            writer.value(*view)?;
            writer.values(indices, MAX_RANKED_MEMORY_RANK)?;
            17
        }
        Op::PredicatedAccess {
            kind,
            view,
            index,
            success,
        } => {
            writer.u16(18)?;
            writer.u16(0)?;
            writer.access(*kind)?;
            writer.value(*view)?;
            writer.value(*index)?;
            writer.value(*success)?;
            18
        }
        Op::ValueAccess {
            kind,
            view,
            indices,
            value,
        } => {
            writer.u16(19)?;
            writer.u16(0)?;
            writer.access(*kind)?;
            writer.value(*view)?;
            writer.values(indices, MAX_RANKED_MEMORY_RANK)?;
            writer.value(*value)?;
            19
        }
        Op::AtomicAccess {
            kind,
            ordering,
            scope,
            view,
            indices,
        } => {
            writer.u16(20)?;
            writer.u16(0)?;
            writer.access(*kind)?;
            writer.ordering(*ordering)?;
            writer.atomic_scope(*scope)?;
            writer.value(*view)?;
            writer.values(indices, MAX_RANKED_MEMORY_RANK)?;
            20
        }
        Op::AtomicValueAccess {
            kind,
            ordering,
            scope,
            view,
            indices,
            value,
        } => {
            writer.u16(21)?;
            writer.u16(0)?;
            writer.access(*kind)?;
            writer.ordering(*ordering)?;
            writer.atomic_scope(*scope)?;
            writer.value(*view)?;
            writer.values(indices, MAX_RANKED_MEMORY_RANK)?;
            writer.value(*value)?;
            21
        }
        Op::OwnershipContract {
            view,
            coverage,
            partition,
        } => {
            writer.u16(22)?;
            writer.u16(0)?;
            writer.value(*view)?;
            writer.coverage(*coverage)?;
            writer.partition(*partition)?;
            22
        }
        Op::AllocationEffect {
            kind,
            memory_space,
            allocation_origin,
            noalias_class,
        } => {
            writer.u16(23)?;
            writer.u16(0)?;
            writer.access(*kind)?;
            writer.memory_space(*memory_space)?;
            writer.u64(*allocation_origin)?;
            writer.u64(*noalias_class)?;
            23
        }
        Op::Barrier {
            execution_scope,
            memory_scope,
            address_space,
            order,
        } => {
            writer.u16(24)?;
            writer.u16(0)?;
            writer.hierarchy(*execution_scope)?;
            writer.memory_scope(*memory_scope)?;
            writer.address_space(*address_space)?;
            writer.memory_order(*order)?;
            24
        }
        Op::Fence {
            memory_scope,
            address_space,
            order,
        } => {
            writer.u16(25)?;
            writer.u16(0)?;
            writer.memory_scope(*memory_scope)?;
            writer.address_space(*address_space)?;
            writer.memory_order(*order)?;
            25
        }
        Op::TensorLayout {
            contract,
            convergence,
            active_lanes,
            binding,
        } => {
            writer.u16(26)?;
            writer.u16(0)?;
            writer.tensor(contract)?;
            writer.convergence(*convergence)?;
            writer.u32(*active_lanes)?;
            writer.cooperative(*binding)?;
            26
        }
        Op::TensorResultComponent {
            result,
            tensor_result_root,
            component,
            scalar,
            numerical_contract,
        } => {
            writer.u16(27)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            writer.digest(*tensor_result_root)?;
            writer.u16(*component)?;
            writer.scalar(*scalar)?;
            writer.numerical(*numerical_contract)?;
            27
        }
        Op::SemanticSymbol { result, symbol } => {
            writer.u16(28)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            writer.u32(*symbol)?;
            28
        }
        Op::SemanticConstant { result, value } => {
            writer.u16(29)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            writer.u64(*value)?;
            29
        }
        Op::SemanticBinary {
            result,
            kind,
            lhs,
            rhs,
        } => {
            writer.u16(30)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            writer.semantic_binary(*kind)?;
            writer.value(*lhs)?;
            writer.value(*rhs)?;
            30
        }
        Op::SemanticExpression {
            result,
            expression,
            numerical_contract,
        } => {
            writer.u16(31)?;
            writer.u16(0)?;
            writer.u32(result.get())?;
            writer.expression(expression)?;
            writer.numerical(*numerical_contract)?;
            31
        }
        Op::CollectiveSemantics {
            contract,
            view,
            actual,
            expected,
            witness0,
            witness1,
        } => {
            writer.u16(32)?;
            writer.u16(0)?;
            writer.collective(contract)?;
            writer.value(*view)?;
            writer.value(*actual)?;
            writer.value(*expected)?;
            writer.value(*witness0)?;
            writer.value(*witness1)?;
            32
        }
        Op::RequireEquivalent { actual, expected } => {
            writer.u16(33)?;
            writer.u16(0)?;
            writer.value(*actual)?;
            writer.value(*expected)?;
            33
        }
        Op::RequireAuthenticatedReferenceEquivalent {
            actual,
            expected,
            proof,
        } => {
            writer.u16(34)?;
            writer.u16(0)?;
            writer.value(*actual)?;
            writer.value(*expected)?;
            writer.proof(*proof)?;
            34
        }
        Op::RequestAuthenticatedReferenceEquivalent {
            actual,
            expected,
            subjects,
        } => {
            writer.u16(35)?;
            writer.u16(0)?;
            writer.value(*actual)?;
            writer.value(*expected)?;
            writer.subjects(*subjects)?;
            35
        }
        Op::RequireEffectRefinement { contract, proof } => {
            writer.u16(36)?;
            writer.u16(0)?;
            writer.effect(contract)?;
            writer.proof(*proof)?;
            36
        }
        Op::RequestEffectRefinement { contract, subjects } => {
            writer.u16(37)?;
            writer.u16(0)?;
            writer.effect(contract)?;
            writer.subjects(*subjects)?;
            37
        }
        Op::RequireNumericalRefinement { contract, proof } => {
            writer.u16(38)?;
            writer.u16(0)?;
            writer.numerical_refinement(*contract)?;
            writer.proof(*proof)?;
            38
        }
        Op::RequestNumericalRefinement { contract, subjects } => {
            writer.u16(39)?;
            writer.u16(0)?;
            writer.numerical_refinement(*contract)?;
            writer.subjects(*subjects)?;
            39
        }
        Op::RequireTensorRefinement { contract, proof } => {
            writer.u16(40)?;
            writer.u16(0)?;
            writer.tensor_refinement(contract)?;
            writer.proof(*proof)?;
            40
        }
        Op::RequestTensorRefinement { contract, subjects } => {
            writer.u16(41)?;
            writer.u16(0)?;
            writer.tensor_refinement(contract)?;
            writer.subjects(*subjects)?;
            41
        }
    };
    writer
        .shape
        .operation(tag, writer.shape.expressions - before)
}
pub(super) fn terminator(writer: &mut Writer<'_, '_>, terminator: &Term) -> R<()> {
    let tag = match terminator {
        Term::IndexLessThan {
            lhs,
            rhs,
            true_block,
            false_block,
        } => {
            writer.u16(1)?;
            writer.u16(0)?;
            writer.value(*lhs)?;
            writer.value(*rhs)?;
            writer.u32(*true_block)?;
            writer.u32(*false_block)?;
            1
        }
        Term::IndexLessThanArgs {
            lhs,
            rhs,
            true_arguments,
            false_arguments,
            true_block,
            false_block,
        } => {
            writer.u16(2)?;
            writer.u16(0)?;
            writer.value(*lhs)?;
            writer.value(*rhs)?;
            writer.values(true_arguments, HARD_MAX_PRODUCTION_RANKED_ARGUMENTS)?;
            writer.values(false_arguments, HARD_MAX_PRODUCTION_RANKED_ARGUMENTS)?;
            writer.u32(*true_block)?;
            writer.u32(*false_block)?;
            2
        }
        Term::IndexEqual {
            lhs,
            rhs,
            true_block,
            false_block,
        } => {
            writer.u16(3)?;
            writer.u16(0)?;
            writer.value(*lhs)?;
            writer.value(*rhs)?;
            writer.u32(*true_block)?;
            writer.u32(*false_block)?;
            3
        }
        Term::IndexEqualArgs {
            lhs,
            rhs,
            true_arguments,
            false_arguments,
            true_block,
            false_block,
        } => {
            writer.u16(4)?;
            writer.u16(0)?;
            writer.value(*lhs)?;
            writer.value(*rhs)?;
            writer.values(true_arguments, HARD_MAX_PRODUCTION_RANKED_ARGUMENTS)?;
            writer.values(false_arguments, HARD_MAX_PRODUCTION_RANKED_ARGUMENTS)?;
            writer.u32(*true_block)?;
            writer.u32(*false_block)?;
            4
        }
        Term::AnalysisSplit {
            control_dependencies,
            first_block,
            second_block,
        } => {
            writer.u16(5)?;
            writer.u16(0)?;
            writer.values(control_dependencies, MAX_RANKED_RECIPE_BYTES_V1 / 6)?;
            writer.u32(*first_block)?;
            writer.u32(*second_block)?;
            5
        }
        Term::AnalysisSplitArgs {
            control_dependencies,
            first_arguments,
            second_arguments,
            first_block,
            second_block,
        } => {
            writer.u16(6)?;
            writer.u16(0)?;
            writer.values(control_dependencies, MAX_RANKED_RECIPE_BYTES_V1 / 6)?;
            writer.values(first_arguments, HARD_MAX_PRODUCTION_RANKED_ARGUMENTS)?;
            writer.values(second_arguments, HARD_MAX_PRODUCTION_RANKED_ARGUMENTS)?;
            writer.u32(*first_block)?;
            writer.u32(*second_block)?;
            6
        }
        Term::Branch { target } => {
            writer.u16(7)?;
            writer.u16(0)?;
            writer.u32(*target)?;
            7
        }
        Term::BranchArgs { arguments, target } => {
            writer.u16(8)?;
            writer.u16(0)?;
            writer.values(arguments, HARD_MAX_PRODUCTION_RANKED_ARGUMENTS)?;
            writer.u32(*target)?;
            8
        }
        Term::BranchArgsAdd {
            value,
            step,
            target,
        } => {
            writer.u16(9)?;
            writer.u16(0)?;
            writer.value(*value)?;
            writer.value(*step)?;
            writer.u32(*target)?;
            9
        }
        Term::BranchArgsAddAt {
            arguments,
            add_argument,
            step,
            target,
        } => {
            writer.u16(10)?;
            writer.u16(0)?;
            writer.values(arguments, HARD_MAX_PRODUCTION_RANKED_ARGUMENTS)?;
            writer.u32(*add_argument)?;
            writer.value(*step)?;
            writer.u32(*target)?;
            10
        }
        Term::Return => {
            writer.u16(11)?;
            writer.u16(0)?;

            11
        }
        Term::Trap => {
            writer.u16(12)?;
            writer.u16(0)?;

            12
        }
    };
    writer.shape.terminator(tag)
}

pub(super) fn retained_heap(kernel: &Kernel, visits: &mut usize) -> R<usize> {
    *visits = visits.checked_sub(1).ok_or(Resource::Accounting)?;
    let mut heap = add(
        kernel.function_name.capacity(),
        product(kernel.blocks.capacity(), size_of::<Block>())?,
    )?;
    for block in &kernel.blocks {
        *visits = visits.checked_sub(1).ok_or(Resource::Accounting)?;
        heap = add(heap, product(block.operations.capacity(), size_of::<Op>())?)?;
        for operation in &block.operations {
            *visits = visits.checked_sub(1).ok_or(Resource::Accounting)?;
            heap = add(heap, operation_heap(operation, visits)?)?;
        }
        heap = add(heap, terminator_heap(&block.terminator)?)?;
    }
    Ok(heap)
}
fn operation_heap(operation: &Op, visits: &mut usize) -> R<usize> {
    match operation {
        Op::View {
            shape,
            dynamic_extents,
            ..
        } => Ok(add(
            add(0, product(shape.capacity(), size_of::<u64>())?)?,
            product(dynamic_extents.capacity(), size_of::<Value>())?,
        )?),
        Op::ViewInSpace {
            shape,
            dynamic_extents,
            ..
        } => Ok(add(
            add(0, product(shape.capacity(), size_of::<u64>())?)?,
            product(dynamic_extents.capacity(), size_of::<Value>())?,
        )?),
        Op::DeterministicJoin { dependencies, .. } => Ok(add(
            0,
            product(dependencies.capacity(), size_of::<Value>())?,
        )?),
        Op::Access { indices, .. } => Ok(add(0, product(indices.capacity(), size_of::<Value>())?)?),
        Op::ValueAccess { indices, .. } => {
            Ok(add(0, product(indices.capacity(), size_of::<Value>())?)?)
        }
        Op::AtomicAccess { indices, .. } => {
            Ok(add(0, product(indices.capacity(), size_of::<Value>())?)?)
        }
        Op::AtomicValueAccess { indices, .. } => {
            Ok(add(0, product(indices.capacity(), size_of::<Value>())?)?)
        }
        Op::SemanticExpression { expression, .. } => {
            Ok(add(0, expression::expression_heap(expression, visits)?)?)
        }
        Op::RequireEffectRefinement { contract, .. } => {
            Ok(add(0, contracts::effect_heap(contract)?)?)
        }
        Op::RequestEffectRefinement { contract, .. } => {
            Ok(add(0, contracts::effect_heap(contract)?)?)
        }
        Op::RequireTensorRefinement { contract, .. } => {
            Ok(add(0, contracts::tensor_heap(contract, visits)?)?)
        }
        Op::RequestTensorRefinement { contract, .. } => {
            Ok(add(0, contracts::tensor_heap(contract, visits)?)?)
        }
        Op::ExecutionLayout { .. }
        | Op::PipelineCreate { .. }
        | Op::PipelineEvent { .. }
        | Op::IndexConstant { .. }
        | Op::IndexUnsignedCast { .. }
        | Op::IndexUnknown { .. }
        | Op::InvocationIndex { .. }
        | Op::IndexBinary { .. }
        | Op::CheckedTiledIndex2D { .. }
        | Op::CheckedRowStripedIndex2D { .. }
        | Op::PredicatedCheckedTiledIndex2D { .. }
        | Op::PredicatedCheckedRowStripedIndex2D { .. }
        | Op::Dimension { .. }
        | Op::PredicatedAccess { .. }
        | Op::OwnershipContract { .. }
        | Op::AllocationEffect { .. }
        | Op::Barrier { .. }
        | Op::Fence { .. }
        | Op::TensorLayout { .. }
        | Op::TensorResultComponent { .. }
        | Op::SemanticSymbol { .. }
        | Op::SemanticConstant { .. }
        | Op::SemanticBinary { .. }
        | Op::CollectiveSemantics { .. }
        | Op::RequireEquivalent { .. }
        | Op::RequireAuthenticatedReferenceEquivalent { .. }
        | Op::RequestAuthenticatedReferenceEquivalent { .. }
        | Op::RequireNumericalRefinement { .. }
        | Op::RequestNumericalRefinement { .. } => Ok(0),
    }
}
fn terminator_heap(terminator: &Term) -> R<usize> {
    match terminator {
        Term::IndexLessThanArgs {
            true_arguments,
            false_arguments,
            ..
        } => Ok(add(
            add(0, product(true_arguments.capacity(), size_of::<Value>())?)?,
            product(false_arguments.capacity(), size_of::<Value>())?,
        )?),
        Term::IndexEqualArgs {
            true_arguments,
            false_arguments,
            ..
        } => Ok(add(
            add(0, product(true_arguments.capacity(), size_of::<Value>())?)?,
            product(false_arguments.capacity(), size_of::<Value>())?,
        )?),
        Term::AnalysisSplit {
            control_dependencies,
            ..
        } => Ok(add(
            0,
            product(control_dependencies.capacity(), size_of::<Value>())?,
        )?),
        Term::AnalysisSplitArgs {
            control_dependencies,
            first_arguments,
            second_arguments,
            ..
        } => Ok(add(
            add(
                add(
                    0,
                    product(control_dependencies.capacity(), size_of::<Value>())?,
                )?,
                product(first_arguments.capacity(), size_of::<Value>())?,
            )?,
            product(second_arguments.capacity(), size_of::<Value>())?,
        )?),
        Term::BranchArgs { arguments, .. } => {
            Ok(add(0, product(arguments.capacity(), size_of::<Value>())?)?)
        }
        Term::BranchArgsAddAt { arguments, .. } => {
            Ok(add(0, product(arguments.capacity(), size_of::<Value>())?)?)
        }
        Term::IndexLessThan { .. }
        | Term::IndexEqual { .. }
        | Term::Branch { .. }
        | Term::BranchArgsAdd { .. }
        | Term::Return
        | Term::Trap => Ok(0),
    }
}
