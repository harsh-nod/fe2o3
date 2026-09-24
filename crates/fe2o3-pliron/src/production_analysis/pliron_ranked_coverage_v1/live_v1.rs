use super::{
    CheckResult, Failure, Fault, Meter, OperationView, Reader, Selection, Site, TerminatorView,
    ordinal,
};
use crate::PRODUCTION_SEMANTIC_LOAD_SYMBOL_BASE_V2;
use crate::production_analysis::{
    ProductionAnalysisInputCensusV1,
    pliron_control_edges_v1::ControlViewV1,
    pliron_function_inventory::{BoundedPlironFunctionInventoryV1, PlironOperationSiteV1},
};
use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};
use dialect_kernel::{
    AccessKindAttr, BranchOp, DYNAMIC_EXTENT, IndexConstantOp, IndexEqualBranchOp,
    IndexLessThanBranchOp, IndexType, IndexUnknownOp, InvocationIndexOp, MAX_RANKED_MEMORY_RANK,
    MemorySpaceAttr, OwnershipContractOp, OwnershipCoverageAttr, OwnershipPartitionAttr,
    RankedAccessOp, RankedViewOp, RankedViewType, ReturnOp, SUPPORTED_ELEMENT_WIDTHS,
    SemanticConstantOp, SemanticExceptionalValueAttr, SemanticIeeeRoundingAttr,
    SemanticNumericalPolicyAttr, SemanticOverflowAttr, SemanticScalarType, SemanticSymbolOp,
    SemanticTypedBinaryKindAttr, SemanticTypedBinaryOp, SemanticTypedCastOp,
    SemanticTypedCompareOp, SemanticTypedConstantOp, SemanticTypedExpressionRootOp,
    SemanticTypedScalarV1, SemanticTypedSelectOp, SemanticTypedSymbolOp,
    SemanticTypedUnaryKindAttr, SemanticTypedUnaryOp, TrapOp,
};
use dialect_proof::{
    CoveredBoundaryAttr, EvidenceRefOp, EvidenceStatusAttr, ObligationOp, PropertyAttr,
    RequireEffectRefinementOp,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{
        ATTR_KEY_DEBUG_INFO, ops::FuncOp, type_interfaces::FunctionTypeInterface,
        types::FunctionType,
    },
    context::{Context, Ptr},
    linked_list::{ContainsLinkedList, LinkedList},
    op::Op,
    operation::Operation,
    r#type::{Type, TypeHandle, Typed},
    value::{DefiningEntity, Value},
};
use std::cell::Ref;

type Inventory = BoundedPlironFunctionInventoryV1;
type OpSite = PlironOperationSiteV1;

pub(super) struct LiveReader<'a> {
    context: &'a Context,
    inventory: &'a Inventory,
    epoch: u64,
}

pub(super) struct PreparedLive<'a> {
    pub(super) reader: LiveReader<'a>,
    pub(super) selected: Selection<Value>,
    #[cfg(test)]
    pub(super) ownership: Site,
    pub(super) view: Value,
}

struct ViewInfo {
    definition: OpSite,
    rank: usize,
    writable: bool,
    single_dynamic: bool,
    space: MemorySpaceAttr,
}

fn require<E>(condition: bool) -> CheckResult<(), E> {
    if condition {
        Ok(())
    } else {
        Err(Fault::Coordinate.into())
    }
}

fn operation_fault(site: OpSite) -> Result<Fault, Fault> {
    Ok(Fault::UnsupportedOperation {
        block: ordinal(site.block())?,
        operation: ordinal(site.operation())?,
    })
}

fn shape_error<E>(error: Failure<E>, fault: Fault) -> Failure<E> {
    match error {
        Failure::Resource(error) => Failure::Resource(error),
        Failure::Rule(Fault::Arithmetic) => Failure::Rule(Fault::Arithmetic),
        Failure::Rule(_) => Failure::Rule(fault),
    }
}

fn nonzero(identity: Option<[u64; 4]>) -> bool {
    identity.is_some_and(|words| words != [0; 4])
}

impl<'a> LiveReader<'a> {
    pub(super) fn check_epoch<M: Meter>(&self, meter: &mut M) -> CheckResult<(), M::Error> {
        meter.charge(1)?;
        let current = self
            .context
            .ir_mutation_attempt_epoch()
            .map_err(|_| Fault::Coordinate)?;
        require(current.value() == self.epoch)
    }

    fn site<M: Meter>(&self, b: usize, o: usize, meter: &mut M) -> CheckResult<OpSite, M::Error> {
        meter.charge(2)?;
        require(b < self.inventory.blocks().len())?;
        let site = self
            .inventory
            .block_operations(b)
            .get(o)
            .copied()
            .ok_or(Fault::Coordinate)?;
        require(site.block() == b && site.operation() == o)?;
        Ok(site)
    }

    fn find_operation<M: Meter>(
        &self,
        pointer: Ptr<Operation>,
        meter: &mut M,
    ) -> CheckResult<OpSite, M::Error> {
        meter.charge(1)?;
        for site in self.inventory.operations() {
            meter.charge(1)?;
            if site.pointer() == pointer {
                return Ok(*site);
            }
        }
        Err(Fault::Coordinate.into())
    }

    fn find_block<M: Meter>(
        &self,
        pointer: Ptr<BasicBlock>,
        meter: &mut M,
    ) -> CheckResult<usize, M::Error> {
        meter.charge(1)?;
        for (index, block) in self.inventory.blocks().iter().enumerate() {
            meter.charge(1)?;
            if *block == pointer {
                return Ok(index);
            }
        }
        Err(Fault::Coordinate.into())
    }

    fn raw<M: Meter>(
        &self,
        site: OpSite,
        meter: &mut M,
    ) -> CheckResult<Ref<'a, Operation>, M::Error> {
        meter.charge(2)?;
        site.pointer()
            .try_deref(self.context)
            .map_err(|_| Fault::Coordinate.into())
    }

    fn shape<M: Meter>(
        &self,
        site: OpSite,
        operands: usize,
        results: usize,
        successors: usize,
        attributes: usize,
        meter: &mut M,
    ) -> CheckResult<(), M::Error> {
        meter.charge(8)?;
        let raw = self.raw(site, meter)?;
        require(
            raw.get_num_operands() == operands
                && raw.get_num_results() == results
                && raw.get_num_successors() == successors
                && raw.num_regions() == 0,
        )?;
        let mut count = 0usize;
        for key in raw.attributes.0.keys() {
            let text: &str = key.as_ref();
            meter.charge(text.len().checked_add(1).ok_or(Fault::Arithmetic)?)?;
            if key != &*ATTR_KEY_DEBUG_INFO {
                count = count.checked_add(1).ok_or(Fault::Arithmetic)?;
            }
        }
        require(count == attributes)
    }

    fn value_type<M: Meter>(
        &self,
        value: Value,
        meter: &mut M,
    ) -> CheckResult<TypeHandle, M::Error> {
        meter.charge(2)?;
        match value.defining_entity() {
            DefiningEntity::Op(pointer) => {
                let site = self.find_operation(pointer, meter)?;
                let raw = self.raw(site, meter)?;
                for index in 0..raw.get_num_results() {
                    meter.charge(1)?;
                    if raw.get_result(index) == value {
                        return Ok(raw.get_type(index));
                    }
                }
            }
            DefiningEntity::Block(pointer) => {
                self.find_block(pointer, meter)?;
                meter.charge(2)?;
                let block = pointer
                    .try_deref(self.context)
                    .map_err(|_| Fault::Coordinate)?;
                let count = block.get_num_arguments();
                for index in 0..count {
                    meter.charge(1)?;
                    if block.get_argument(index) == value {
                        // Value::get_type performs another argument-index scan.
                        meter.charge(count.checked_add(2).ok_or(Fault::Arithmetic)?)?;
                        return Ok(value.get_type(self.context));
                    }
                }
            }
        }
        Err(Fault::Coordinate.into())
    }

    fn value_is<T: Type + 'static, M: Meter>(
        &self,
        value: Value,
        meter: &mut M,
    ) -> CheckResult<(), M::Error> {
        let handle = self.value_type(value, meter)?;
        meter.charge(2)?;
        let ty = handle
            .try_deref(self.context)
            .map_err(|_| Fault::Coordinate)?;
        require(ty.is::<T>())
    }

    fn result_is<T: Type + 'static, M: Meter>(
        &self,
        site: OpSite,
        meter: &mut M,
    ) -> CheckResult<(), M::Error> {
        meter.charge(2)?;
        let raw = self.raw(site, meter)?;
        require(raw.get_num_results() == 1)?;
        let ty = raw
            .get_type(0)
            .try_deref(self.context)
            .map_err(|_| Fault::Coordinate)?;
        require(ty.is::<T>())
    }

    fn validate_roster<M: Meter>(
        &self,
        function: &FuncOp,
        census: ProductionAnalysisInputCensusV1,
        meter: &mut M,
    ) -> CheckResult<(), M::Error> {
        self.check_epoch(meter)?;
        meter.charge(8)?;
        let blocks = self.inventory.blocks();
        require(
            !blocks.is_empty()
                && blocks.len() <= census.blocks
                && self.inventory.operations().len() <= census.operations,
        )?;

        // The authenticated function is the anchor, not an inventory operation.
        let function_pointer = function.get_operation();
        let function_raw = function_pointer
            .try_deref(self.context)
            .map_err(|_| Fault::Coordinate)?;
        require(
            Operation::is_op::<FuncOp>(function_pointer, self.context)
                && function_raw.num_regions() == 1
                && function_raw.get_num_operands() == 0
                && function_raw.get_num_results() == 0
                && function_raw.get_num_successors() == 0,
        )?;
        let region_pointer = function_raw.get_region(0);
        let region = region_pointer
            .try_deref(self.context)
            .map_err(|_| Fault::Coordinate)?;
        require(
            region.get_parent_op() == function_pointer
                && region.get_head() == blocks.first().copied()
                && region.get_tail() == blocks.last().copied(),
        )?;

        for (b, pointer) in blocks.iter().copied().enumerate() {
            meter.charge(8)?;
            let block = pointer
                .try_deref(self.context)
                .map_err(|_| Fault::Coordinate)?;
            require(
                block.get_parent_region() == Some(region_pointer)
                    && block.get_prev() == b.checked_sub(1).map(|i| blocks[i])
                    && block.get_next() == blocks.get(b + 1).copied(),
            )?;
            if b != 0 && block.get_num_arguments() != 0 {
                return Err(Fault::UnsupportedTerminator { block: ordinal(b)? }.into());
            }
            let operations = self.inventory.block_operations(b);
            if operations.is_empty() {
                return Err(Fault::UnsupportedTerminator { block: ordinal(b)? }.into());
            }
            require(
                block.get_head() == operations.first().map(|s| s.pointer())
                    && block.get_tail() == operations.last().map(|s| s.pointer()),
            )?;
            for (o, site) in operations.iter().copied().enumerate() {
                meter.charge(8)?;
                require(site.block() == b && site.operation() == o)?;
                let raw = self.raw(site, meter)?;
                require(
                    raw.get_parent_block() == Some(pointer)
                        && raw.get_prev() == o.checked_sub(1).map(|i| operations[i].pointer())
                        && raw.get_next() == operations.get(o + 1).map(|s| s.pointer()),
                )?;
            }
        }

        // Storage for these two owned API results must already be admitted.
        meter.charge(
            census
                .type_nodes
                .checked_mul(4)
                .and_then(|n| n.checked_add(8))
                .ok_or(Fault::Arithmetic)?,
        )?;
        let attribute = function
            .get_attr_func_type(self.context)
            .ok_or(Fault::Coordinate)?;
        let handle = attribute.get_type(self.context);
        let borrowed = handle
            .try_deref(self.context)
            .map_err(|_| Fault::Coordinate)?;
        let signature = borrowed
            .downcast_ref::<FunctionType>()
            .ok_or(Fault::Coordinate)?;
        let arguments = signature.arg_types();
        let results = signature.res_types();
        let items = arguments
            .len()
            .checked_add(results.len())
            .ok_or(Fault::Arithmetic)?;
        require(items <= census.type_nodes && results.is_empty())?;

        let entry = blocks[0]
            .try_deref(self.context)
            .map_err(|_| Fault::Coordinate)?;
        require(
            entry.get_num_arguments() == arguments.len()
                && arguments.len() <= census.block_arguments,
        )?;
        for (index, expected) in arguments.iter().copied().enumerate() {
            meter.charge(4)?;
            let actual = self.value_type(entry.get_argument(index), meter)?;
            require(actual == expected)?;
            let ty = expected
                .try_deref(self.context)
                .map_err(|_| Fault::Coordinate)?;
            require(ty.is::<IndexType>())?;
        }
        self.check_epoch(meter)
    }

    fn view_info<M: Meter>(&self, value: Value, meter: &mut M) -> CheckResult<ViewInfo, M::Error> {
        meter.charge(16)?;
        let pointer = value.defining_op().ok_or(Fault::Coordinate)?;
        let site = self.find_operation(pointer, meter)?;
        let raw = self.raw(site, meter)?;
        require(raw.get_num_results() == 1 && raw.get_result(0) == value)?;
        let view =
            Operation::get_op::<RankedViewOp>(pointer, self.context).ok_or(Fault::Coordinate)?;
        let has_origin = view
            .get_attr_kernel_allocation_origin(self.context)
            .is_some();
        let has_class = view.get_attr_kernel_noalias_class(self.context).is_some();
        require(has_origin == has_class)?;

        let ty = raw
            .get_type(0)
            .try_deref(self.context)
            .map_err(|_| Fault::Coordinate)?;
        let ty = ty
            .downcast_ref::<RankedViewType>()
            .ok_or(Fault::Coordinate)?;
        let rank = ty.rank();
        require((1..=MAX_RANKED_MEMORY_RANK).contains(&rank))?;
        meter.charge(rank.checked_add(8).ok_or(Fault::Arithmetic)?)?;
        require(SUPPORTED_ELEMENT_WIDTHS.contains(&ty.element_width()))?;
        let dynamic = ty.shape().iter().filter(|&&n| n == DYNAMIC_EXTENT).count();
        self.shape(site, dynamic, 1, 0, if has_origin { 3 } else { 1 }, meter)?;

        let space = view.memory_space(self.context).ok_or(Fault::Coordinate)?;
        let origin = view
            .allocation_origin(self.context)
            .ok_or(Fault::Coordinate)?;
        let class = view.noalias_class(self.context).ok_or(Fault::Coordinate)?;
        require(class == 0 || origin != 0)?;
        for index in 0..dynamic {
            meter.charge(1)?;
            self.value_is::<IndexType, _>(raw.get_operand(index), meter)?;
        }
        Ok(ViewInfo {
            definition: site,
            rank,
            writable: ty.writable(),
            single_dynamic: ty.shape() == [DYNAMIC_EXTENT],
            space,
        })
    }

    fn constant<M: Meter>(
        &self,
        site: OpSite,
        meter: &mut M,
    ) -> CheckResult<(Value, u64), M::Error> {
        self.shape(site, 0, 1, 0, 1, meter)?;
        self.result_is::<IndexType, _>(site, meter)?;
        let op = Operation::get_op::<IndexConstantOp>(site.pointer(), self.context)
            .ok_or(Fault::Coordinate)?;
        let literal = op.value(self.context).ok_or(Fault::Coordinate)?;
        Ok((op.result(self.context), literal))
    }

    fn typed_leaf<M: Meter>(
        &self,
        site: OpSite,
        meter: &mut M,
    ) -> CheckResult<SemanticTypedScalarV1, M::Error> {
        meter.charge(8)?;
        self.shape(site, 0, 1, 0, 3, meter)?;
        self.result_is::<SemanticScalarType, _>(site, meter)?;
        if let Some(op) = Operation::get_op::<SemanticTypedSymbolOp>(site.pointer(), self.context) {
            // Reserved load symbols are bound by the constructor/source replay
            // and mandatory semantic analysis, not treated as coverage proofs.
            op.symbol(self.context).ok_or(Fault::Coordinate)?;
            return op
                .scalar(self.context)
                .ok_or_else(|| Fault::Coordinate.into());
        }
        let op = Operation::get_op::<SemanticTypedConstantOp>(site.pointer(), self.context)
            .ok_or(Fault::Coordinate)?;
        let scalar = op.scalar(self.context).ok_or(Fault::Coordinate)?;
        let bits = op.bits(self.context).ok_or(Fault::Coordinate)?;
        require(scalar.bits() == 64 || bits < (1_u64 << scalar.bits()))?;
        Ok(scalar)
    }

    fn typed_scalar<M: Meter>(
        &self,
        site: OpSite,
        meter: &mut M,
    ) -> CheckResult<SemanticTypedScalarV1, M::Error> {
        meter.charge(8)?;
        let pointer = site.pointer();
        let context = self.context;
        let scalar = if let Some(op) = Operation::get_op::<SemanticTypedBinaryOp>(pointer, context)
        {
            op.scalar(context)
        } else if let Some(op) = Operation::get_op::<SemanticTypedUnaryOp>(pointer, context) {
            op.scalar(context)
        } else if Operation::is_op::<SemanticTypedCompareOp>(pointer, context) {
            SemanticTypedScalarV1::new(dialect_kernel::SemanticScalarKindAttr::Bool, 1)
        } else if let Some(op) = Operation::get_op::<SemanticTypedSelectOp>(pointer, context) {
            op.scalar(context)
        } else if let Some(op) = Operation::get_op::<SemanticTypedCastOp>(pointer, context) {
            op.target(context)
        } else {
            return self.typed_leaf(site, meter);
        };
        scalar.ok_or_else(|| Fault::Coordinate.into())
    }

    fn decode_operation<M: Meter>(
        &self,
        site: OpSite,
        meter: &mut M,
    ) -> CheckResult<OperationView, M::Error> {
        meter.charge(32)?;
        let pointer = site.pointer();
        let context = self.context;
        let raw = self.raw(site, meter)?;

        if Operation::is_op::<IndexConstantOp>(pointer, context) {
            self.constant(site, meter)?;
        } else if Operation::is_op::<IndexUnknownOp>(pointer, context) {
            self.shape(site, 0, 1, 0, 0, meter)?;
            self.result_is::<IndexType, _>(site, meter)?;
        } else if let Some(op) = Operation::get_op::<InvocationIndexOp>(pointer, context) {
            self.shape(site, 0, 1, 0, 2, meter)?;
            self.result_is::<IndexType, _>(site, meter)?;
            require(
                op.dimension(context)
                    .is_some_and(|d| d < MAX_RANKED_MEMORY_RANK as u32)
                    && op.launch_extent(context).is_some(),
            )?;
        } else if Operation::is_op::<RankedViewOp>(pointer, context) {
            require(raw.get_num_results() == 1)?;
            self.view_info(raw.get_result(0), meter)?;
        } else if let Some(op) = Operation::get_op::<RankedAccessOp>(pointer, context) {
            require(raw.get_num_operands() != 0)?;
            let view = self.view_info(raw.get_operand(0), meter)?;
            self.shape(site, view.rank + 1, 0, 0, 1, meter)?;
            let kind = op.kind(context).ok_or(Fault::Coordinate)?;
            require(
                (kind == AccessKindAttr::Read || (view.writable && kind == AccessKindAttr::Write))
                    && op.atomic_ordering(context).is_none()
                    && op.atomic_scope(context).is_none(),
            )?;
            for index in 1..=view.rank {
                meter.charge(1)?;
                self.value_is::<IndexType, _>(raw.get_operand(index), meter)?;
            }
            return Ok(if kind == AccessKindAttr::Read {
                OperationView::Read
            } else {
                OperationView::Write
            });
        } else if let Some(op) = Operation::get_op::<OwnershipContractOp>(pointer, context) {
            self.shape(site, 1, 0, 0, 2, meter)?;
            require(op.coverage(context).is_some() && op.partition(context).is_some())?;
            let view = self.view_info(raw.get_operand(0), meter)?;
            require(view.writable && view.space == MemorySpaceAttr::Global)?;
        } else if let Some(op) = Operation::get_op::<SemanticConstantOp>(pointer, context) {
            self.shape(site, 0, 1, 0, 1, meter)?;
            self.result_is::<SemanticScalarType, _>(site, meter)?;
            require(op.value(context).is_some())?;
        } else if let Some(op) = Operation::get_op::<SemanticSymbolOp>(pointer, context) {
            self.shape(site, 0, 1, 0, 1, meter)?;
            self.result_is::<SemanticScalarType, _>(site, meter)?;
            require(
                op.symbol(context)
                    .is_some_and(|s| s < PRODUCTION_SEMANTIC_LOAD_SYMBOL_BASE_V2),
            )?;
        } else if Operation::is_op::<SemanticTypedConstantOp>(pointer, context)
            || Operation::is_op::<SemanticTypedSymbolOp>(pointer, context)
        {
            self.typed_leaf(site, meter)?;
        } else if let Some(op) = Operation::get_op::<SemanticTypedBinaryOp>(pointer, context) {
            self.shape(site, 2, 1, 0, 4, meter)?;
            self.result_is::<SemanticScalarType, _>(site, meter)?;
            require(
                op.scalar(context).is_some()
                    && op.overflow(context) == Some(SemanticOverflowAttr::Wrapping)
                    && matches!(
                        op.kind(context),
                        Some(
                            SemanticTypedBinaryKindAttr::Add
                                | SemanticTypedBinaryKindAttr::Subtract
                                | SemanticTypedBinaryKindAttr::Multiply
                                | SemanticTypedBinaryKindAttr::BitAnd
                                | SemanticTypedBinaryKindAttr::BitOr
                                | SemanticTypedBinaryKindAttr::BitXor
                        )
                    ),
            )?;
            for index in 0..2 {
                self.value_is::<SemanticScalarType, _>(raw.get_operand(index), meter)?;
            }
        } else if let Some(op) = Operation::get_op::<SemanticTypedUnaryOp>(pointer, context) {
            self.shape(site, 1, 1, 0, 3, meter)?;
            self.result_is::<SemanticScalarType, _>(site, meter)?;
            self.value_is::<SemanticScalarType, _>(raw.get_operand(0), meter)?;
            let scalar = op.scalar(context).ok_or(Fault::Coordinate)?;
            require(
                op.kind(context) == Some(SemanticTypedUnaryKindAttr::Not)
                    || (op.kind(context) == Some(SemanticTypedUnaryKindAttr::Negate)
                        && scalar.is_float()),
            )?;
        } else if let Some(op) = Operation::get_op::<SemanticTypedCompareOp>(pointer, context) {
            self.shape(site, 2, 1, 0, 3, meter)?;
            self.result_is::<SemanticScalarType, _>(site, meter)?;
            require(op.kind(context).is_some() && op.operand_scalar(context).is_some())?;
            for index in 0..2 {
                self.value_is::<SemanticScalarType, _>(raw.get_operand(index), meter)?;
            }
        } else if let Some(op) = Operation::get_op::<SemanticTypedSelectOp>(pointer, context) {
            self.shape(site, 3, 1, 0, 2, meter)?;
            self.result_is::<SemanticScalarType, _>(site, meter)?;
            require(op.scalar(context).is_some())?;
            for index in 0..3 {
                self.value_is::<SemanticScalarType, _>(raw.get_operand(index), meter)?;
            }
        } else if let Some(op) = Operation::get_op::<SemanticTypedCastOp>(pointer, context) {
            self.shape(site, 1, 1, 0, 5, meter)?;
            self.result_is::<SemanticScalarType, _>(site, meter)?;
            self.value_is::<SemanticScalarType, _>(raw.get_operand(0), meter)?;
            require(
                op.kind(context).is_some()
                    && op.source(context).is_some()
                    && op.target(context).is_some(),
            )?;
        } else if let Some(op) =
            Operation::get_op::<SemanticTypedExpressionRootOp>(pointer, context)
        {
            self.shape(site, 1, 1, 0, 4, meter)?;
            self.result_is::<SemanticScalarType, _>(site, meter)?;
            let operand = raw.get_operand(0);
            self.value_is::<SemanticScalarType, _>(operand, meter)?;
            let definition = operand.defining_op().ok_or(Fault::Coordinate)?;
            let leaf_site = self.find_operation(definition, meter)?;
            let leaf_raw = self.raw(leaf_site, meter)?;
            require(leaf_raw.get_num_results() == 1 && leaf_raw.get_result(0) == operand)?;
            let scalar = self.typed_scalar(leaf_site, meter)?;
            let policy = if scalar.is_float() {
                SemanticNumericalPolicyAttr::ExactIeeeNearestTiesToEvenPreserveBits
            } else {
                SemanticNumericalPolicyAttr::ExactBitVectorOperatorCongruence
            };
            require(
                op.policy(context) == Some(policy)
                    && op.rounding(context) == Some(SemanticIeeeRoundingAttr::NearestTiesToEven)
                    && op.exceptional_values(context)
                        == Some(SemanticExceptionalValueAttr::PreserveExactBits)
                    && nonzero(op.commitment(context)),
            )?;
        } else if let Some(op) = Operation::get_op::<ObligationOp>(pointer, context) {
            self.shape(site, 0, 0, 0, 4, meter)?;
            require(
                nonzero(op.obligation_id(context))
                    && nonzero(op.subject_id(context))
                    && nonzero(op.model_id(context))
                    && op.property(context) == Some(PropertyAttr::FunctionalRefinement),
            )?;
        } else if let Some(op) = Operation::get_op::<EvidenceRefOp>(pointer, context) {
            self.shape(site, 0, 0, 0, 5, meter)?;
            require(
                nonzero(op.evidence_id(context))
                    && nonzero(op.obligation_id(context))
                    && op.evidence_id(context) != op.obligation_id(context)
                    && op.property(context) == Some(PropertyAttr::FunctionalRefinement)
                    && op.status(context) == Some(EvidenceStatusAttr::Checked)
                    && op.covered_boundary(context) == Some(CoveredBoundaryAttr::Mir),
            )?;
        } else if let Some(op) = Operation::get_op::<RequireEffectRefinementOp>(pointer, context) {
            require(raw.get_num_operands() != 0)?;
            let view = self.view_info(raw.get_operand(0), meter)?;
            let operands = view
                .rank
                .checked_mul(3)
                .and_then(|n| n.checked_add(7))
                .ok_or(Fault::Arithmetic)?;
            self.shape(site, operands, 0, 0, 1, meter)?;
            require(nonzero(op.obligation_id(context)))?;
            for index in 1..=view.rank {
                meter.charge(1)?;
                self.value_is::<IndexType, _>(raw.get_operand(index), meter)?;
            }
            for index in view.rank + 1..operands {
                meter.charge(1)?;
                self.value_is::<SemanticScalarType, _>(raw.get_operand(index), meter)?;
            }
        } else if let Some(op) = Operation::get_op::<ExecutionLayoutOp>(pointer, context) {
            self.shape(site, 0, 0, 0, 9, meter)?;
            let global = op.global_extents(context).ok_or(Fault::Coordinate)?;
            let workgroup = op.workgroup_extents(context).ok_or(Fault::Coordinate)?;
            let subgroup = op.subgroup_size(context).ok_or(Fault::Coordinate)?;
            let domain = *op
                .get_attr_gpu_execution_domain(context)
                .ok_or(Fault::Coordinate)?;
            require(
                op.grid_identity(context).is_some()
                    && subgroup != 0
                    && !workgroup.contains(&0)
                    && workgroup
                        .into_iter()
                        .try_fold(1_u64, u64::checked_mul)
                        .is_some(),
            )?;
            meter.charge(4)?;
            require(
                domain != ExecutionDomainAttr::FullPhysicalWorkgroups
                    || global
                        .into_iter()
                        .zip(workgroup)
                        .all(|(g, w)| g == 0 || g.is_multiple_of(w)),
            )?;
        } else {
            return Err(Fault::Coordinate.into());
        }
        Ok(OperationView::Inert)
    }

    fn decode_terminator<M: Meter>(
        &self,
        site: OpSite,
        meter: &mut M,
    ) -> CheckResult<TerminatorView<Value, Ptr<BasicBlock>>, M::Error> {
        meter.charge(16)?;
        let pointer = site.pointer();
        let context = self.context;
        let raw = self.raw(site, meter)?;
        let branch = Operation::is_op::<BranchOp>(pointer, context);
        let ret = Operation::is_op::<ReturnOp>(pointer, context);
        let trap = Operation::is_op::<TrapOp>(pointer, context);
        let lt = Operation::is_op::<IndexLessThanBranchOp>(pointer, context);
        let eq = Operation::is_op::<IndexEqualBranchOp>(pointer, context);
        require(branch || ret || trap || lt || eq)?;
        let successors = if branch {
            1
        } else if lt || eq {
            2
        } else {
            0
        };
        self.shape(site, if lt || eq { 2 } else { 0 }, 0, successors, 0, meter)?;

        // ControlView::observe/edge may dereference targets.
        for index in 0..successors {
            meter.charge(2)?;
            let target = raw.get_successor(index);
            self.find_block(target, meter)?;
            let block = target.try_deref(context).map_err(|_| Fault::Coordinate)?;
            require(block.get_num_arguments() == 0)?;
        }
        if lt || eq {
            self.value_is::<IndexType, _>(raw.get_operand(0), meter)?;
            self.value_is::<IndexType, _>(raw.get_operand(1), meter)?;
        }
        meter.charge(8)?;
        let control = ControlViewV1::observe(context, pointer).map_err(|_| Fault::Coordinate)?;
        require(control.successor_count() == successors)?;
        if ret {
            return Ok(TerminatorView::Return);
        }
        if trap {
            return Ok(TerminatorView::Trap);
        }

        let first = control.edge(0).map_err(|_| Fault::Coordinate)?;
        require(first.argument_count() == 0)?;
        if branch {
            return Ok(TerminatorView::Branch(first.target()));
        }
        let second = control.edge(1).map_err(|_| Fault::Coordinate)?;
        require(second.argument_count() == 0)?;
        Ok(TerminatorView::Compare {
            equal: eq,
            lhs: raw.get_operand(0),
            rhs: raw.get_operand(1),
            true_block: first.target(),
            false_block: second.target(),
        })
    }
}

impl Reader for LiveReader<'_> {
    type Value = Value;
    type Target = Ptr<BasicBlock>;

    fn block_count(&self) -> usize {
        self.inventory.blocks().len()
    }

    fn block_argument_count(&self, _block: usize) -> usize {
        // Preparation rejects nonentry arguments. Entry arguments are the ABI.
        // Avoid dereferencing a possibly stale block in this infallible method.
        0
    }

    fn operation_count(&self, block: usize) -> usize {
        self.inventory
            .block_operations(block)
            .len()
            .saturating_sub(1)
    }

    fn is_local(&self, value: Value) -> bool {
        value.defining_op().is_some()
    }

    fn index_constant<M: Meter>(
        &self,
        b: usize,
        o: usize,
        meter: &mut M,
    ) -> CheckResult<Option<(Value, u64)>, M::Error> {
        self.check_epoch(meter)?;
        meter.charge(8)?;
        let site = self.site(b, o, meter)?;
        let _raw = self.raw(site, meter)?;
        if !Operation::is_op::<IndexConstantOp>(site.pointer(), self.context) {
            return Ok(None);
        }
        let fault = operation_fault(site)?;
        self.constant(site, meter)
            .map(Some)
            .map_err(|error| shape_error(error, fault))
    }

    fn operation<M: Meter>(
        &self,
        b: usize,
        o: usize,
        meter: &mut M,
    ) -> CheckResult<OperationView, M::Error> {
        self.check_epoch(meter)?;
        let site = self.site(b, o, meter)?;
        let fault = operation_fault(site)?;
        self.decode_operation(site, meter)
            .map_err(|error| shape_error(error, fault))
    }

    fn terminator<M: Meter>(
        &self,
        b: usize,
        meter: &mut M,
    ) -> CheckResult<TerminatorView<Value, Ptr<BasicBlock>>, M::Error> {
        self.check_epoch(meter)?;
        require(b < self.inventory.blocks().len())?;
        let o = self
            .inventory
            .block_operations(b)
            .len()
            .checked_sub(1)
            .ok_or(Fault::UnsupportedTerminator { block: ordinal(b)? })?;
        let site = self.site(b, o, meter)?;
        let fault = Fault::UnsupportedTerminator { block: ordinal(b)? };
        self.decode_terminator(site, meter)
            .map_err(|error| shape_error(error, fault))
    }

    fn target<M: Meter>(
        &self,
        target: Ptr<BasicBlock>,
        meter: &mut M,
    ) -> CheckResult<u32, M::Error> {
        self.check_epoch(meter)?;
        Ok(ordinal(self.find_block(target, meter)?)?)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare<'a, M: Meter>(
    context: &'a Context,
    function: &FuncOp,
    inventory: &'a Inventory,
    census: ProductionAnalysisInputCensusV1,
    epoch: u64,
    ownership: Ptr<Operation>,
    original_view: Value,
    meter: &mut M,
) -> CheckResult<PreparedLive<'a>, M::Error> {
    let reader = LiveReader {
        context,
        inventory,
        epoch,
    };
    reader.validate_roster(function, census, meter)?;

    let contract_site = reader.find_operation(ownership, meter)?;
    meter.charge(8)?;
    require(contract_site.block() == 0)?;
    let contract_raw = reader.raw(contract_site, meter)?;
    require(contract_raw.get_num_operands() == 1)?;
    let contract =
        Operation::get_op::<OwnershipContractOp>(ownership, context).ok_or(Fault::Coordinate)?;
    require(
        contract_raw.get_operand(0) == original_view
            && contract.coverage(context) == Some(OwnershipCoverageAttr::TotalView)
            && contract.partition(context) == Some(OwnershipPartitionAttr::ExactSets),
    )?;
    reader.operation(contract_site.block(), contract_site.operation(), meter)?;
    let view = reader.view_info(original_view, meter)?;
    require(
        view.writable
            && view.space == MemorySpaceAttr::Global
            && view.rank == 1
            && view.single_dynamic,
    )?;
    let view_raw = reader.raw(view.definition, meter)?;
    let extent = view_raw.get_operand(0);
    require(extent.defining_block() == inventory.blocks().first().copied())?;
    reader.value_is::<IndexType, _>(extent, meter)?;

    let mut write = None;
    for site in inventory.operations().iter().copied() {
        meter.charge(8)?;
        let raw = reader.raw(site, meter)?;
        let Some(access) = Operation::get_op::<RankedAccessOp>(site.pointer(), context) else {
            continue;
        };
        if raw.get_num_operands() == 0
            || raw.get_operand(0) != original_view
            || !access
                .kind(context)
                .is_some_and(AccessKindAttr::writes_memory)
        {
            continue;
        }
        reader.operation(site.block(), site.operation(), meter)?;
        require(raw.get_num_operands() == 2)?;
        write = Some((site, raw.get_operand(1)));
        // The common census rejects additional writes in CFG observation order.
        break;
    }
    let (write_site, index) = write.ok_or(Fault::Coordinate)?;
    let index_pointer = index.defining_op().ok_or(Fault::Coordinate)?;
    let index_site = reader.find_operation(index_pointer, meter)?;
    reader.operation(index_site.block(), index_site.operation(), meter)?;
    let index_raw = reader.raw(index_site, meter)?;
    let invocation =
        Operation::get_op::<InvocationIndexOp>(index_pointer, context).ok_or(Fault::Coordinate)?;
    require(
        index_raw.get_num_results() == 1
            && index_raw.get_result(0) == index
            && invocation.dimension(context) == Some(0)
            && invocation.launch_extent(context) == Some(0),
    )?;
    reader.value_is::<IndexType, _>(index, meter)?;
    reader.check_epoch(meter)?;

    let selected = Selection {
        index,
        extent,
        write: Site {
            block: ordinal(write_site.block())?,
            operation: ordinal(write_site.operation())?,
        },
    };
    let _ownership = Site {
        block: ordinal(contract_site.block())?,
        operation: ordinal(contract_site.operation())?,
    };
    Ok(PreparedLive {
        reader,
        selected,
        #[cfg(test)]
        ownership: _ownership,
        view: original_view,
    })
}
