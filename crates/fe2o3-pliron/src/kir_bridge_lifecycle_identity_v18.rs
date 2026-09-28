// Identity-only extension for the exact immutable V18 import. This is not a
// private-memory certificate or a completed original-source role profile.
// Exact imported Unreachable is a zero-edge terminal, not a no-return call.
pub(crate) struct NativeLifecycleIdentityAdmissionV18<'a> {
    context: &'a Context,
    function: &'a FuncOp,
    origins: &'a KirBridgeOriginsV1,
    ordinal: usize,
    epoch: u64,
    operations: usize,
}

impl NativeLifecycleIdentityAdmissionV18<'_> {
    pub(crate) fn context(&self) -> &Context {
        self.context
    }

    pub(crate) fn function(&self) -> &FuncOp {
        self.function
    }

    pub(crate) fn authenticate(&self, context: &Context, function: &FuncOp) -> bool {
        std::ptr::eq(context, self.context)
            && function.get_operation() == self.function.get_operation()
            && context
                .ir_mutation_attempt_epoch()
                .ok()
                .map(|epoch| epoch.value())
                == Some(self.epoch)
            && self.origins.functions.get(&function.get_operation()) == Some(&self.ordinal)
    }

    pub(crate) fn operation(&self, context: &Context, pointer: Ptr<Operation>) -> Option<()> {
        use fe2o3_kernel_ir::ExecutionOperationV15 as E;
        use pliron::builtin::op_interfaces::OneRegionInterface;
        if !self.authenticate(context, self.function) {
            return None;
        }
        let raw = pointer.deref(context);
        let block = raw.get_parent_block()?;
        if self.origins.blocks.get(&block)?.0 != self.ordinal
            || block.deref(context).get_parent_region() != Some(self.function.get_region(context))
        {
            return None;
        }
        if Operation::is_op::<PreservedTerminatorOp>(pointer, context) {
            return self.unreachable(context, pointer).then_some(());
        }
        if let Some(operation) = Operation::get_op::<PreservedOperationOp>(pointer, context) {
            let Some(OperationKind::Execution(kind)) =
                self.origins.preserved_operations.get(&pointer)
            else {
                return None;
            };
            if operation.kind(context) != Some(PreservedOperationKindAttr::ExecutionV18)
                || raw.num_regions() != 0
                || raw.get_num_successors() != 0
                || raw.attributes.0.len() != 1
            {
                return None;
            }
            let role = |value: Value, expected: u32| {
                let ty = value.get_type(context);
                let ty = ty.deref(context);
                ty.downcast_ref::<dialect_gpu::storage_types_v18::ExecutionRoleTypeV18>()
                    .is_some_and(|role| {
                        role.role() == expected && role.lanes() == 0 && role.elements() == 0
                    })
            };
            let operand = |id: ValueId, expected| {
                self.origins.values.get(&raw.get_operand(0)) == Some(&id)
                    && role(raw.get_operand(0), expected)
            };
            let closed = match kind {
                E::ContextIssue => {
                    raw.get_num_operands() == 0
                        && raw.get_num_results() == 1
                        && role(raw.get_result(0), 1)
                }
                E::WorkgroupDerive { context: id } => {
                    raw.get_num_operands() == 1
                        && raw.get_num_results() == 1
                        && operand(*id, 1)
                        && role(raw.get_result(0), 2)
                }
                E::ScopeEnd {
                    workgroup,
                    discarded,
                } => {
                    discarded.is_empty()
                        && raw.get_num_operands() == 1
                        && raw.get_num_results() == 0
                        && operand(*workgroup, 2)
                }
                _ => false,
            };
            return closed.then_some(());
        }
        crate::is_production_ranked_operation_v1(Operation::get_op_dyn(pointer, context).as_ref())
            .then_some(())
    }

    pub(crate) fn attribute(
        &self,
        context: &Context,
        pointer: Ptr<Operation>,
        key: &str,
        dialect: &str,
        name: &str,
    ) -> bool {
        self.operation(context, pointer).is_some()
            && dialect == "gpu"
            && ((Operation::is_op::<PreservedOperationOp>(pointer, context)
                && key == "gpu_preserved_operation_kind"
                && name == "preserved_operation_kind")
                || (Operation::is_op::<PreservedTerminatorOp>(pointer, context)
                    && key == "gpu_preserved_terminator_kind"
                    && name == "preserved_terminator_kind"))
    }

    pub(crate) fn unreachable(&self, context: &Context, pointer: Ptr<Operation>) -> bool {
        use pliron::builtin::{
            attributes::OperandSegmentSizesAttr,
            op_interfaces::{ATTR_KEY_OPERAND_SEGMENT_SIZES, OneRegionInterface},
        };
        if !self.authenticate(context, self.function)
            || self.origins.preserved_terminators.get(&pointer) != Some(&Terminator::Unreachable)
        {
            return false;
        }
        let Some(op) = Operation::get_op::<PreservedTerminatorOp>(pointer, context) else {
            return false;
        };
        let raw = pointer.deref(context);
        let Some(block) = raw.get_parent_block() else {
            return false;
        };
        let block_ref = block.deref(context);
        self.origins
            .blocks
            .get(&block)
            .is_some_and(|row| row.0 == self.ordinal)
            && block_ref.get_parent_region() == Some(self.function.get_region(context))
            && block_ref.get_terminator(context) == Some(pointer)
            && op.kind(context) == Some(PreservedTerminatorKindAttr::Unreachable)
            && raw.get_num_operands() == 0
            && raw.get_num_results() == 0
            && raw.get_num_successors() == 0
            && raw.num_regions() == 0
            && raw.attributes.0.len() == 2
            && raw
                .attributes
                .get::<OperandSegmentSizesAttr>(&ATTR_KEY_OPERAND_SEGMENT_SIZES)
                .is_some_and(|sizes| sizes.0.is_empty())
    }

    pub(crate) fn identity_lookup_work(&self) -> Option<usize> {
        // Each identity traversal authenticates the original function/block
        // and preserved occurrence through bounded hash tables. Account for
        // their worst-case probe cost, including repeated attribute rendering.
        let probes = self
            .origins
            .functions
            .len()
            .checked_add(self.origins.blocks.len())?
            .checked_add(self.origins.preserved_operations.len())?
            .checked_add(self.origins.preserved_terminators.len())?
            .checked_add(self.origins.values.len())?
            .checked_add(8)?;
        self.operations
            .checked_add(1)?
            .checked_mul(probes)?
            .checked_mul(512)
    }
}
