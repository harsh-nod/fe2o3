//! Native terminal schema is admitted only for a sealed actual graph occurrence.
use super::*;

pub(super) struct NativeTrapModuleV1 {
    definitions: Vec<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>,
    signature: Option<TypeHandle>,
}
impl NativeTrapModuleV1 {
    pub(super) fn capture(
        facts: &CanonicalPrivateGraphFactsV1<'_, '_>,
        context: &Context,
        root: Ptr<Operation>,
        budget: &mut Budget<'_>,
    ) -> Result<Option<Self>, Failure> {
        let Some(terminals) = facts.terminal_facts() else {
            return Ok(None);
        };
        let count = terminals.definitions().len();
        let width = std::mem::size_of::<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>();
        budget.reserve_storage(checked_bridge_add_v12(
            std::mem::size_of::<Self>(),
            count.checked_mul(width).ok_or(Resource::Arithmetic)?,
        )?)?;
        let mut definitions = Vec::new();
        definitions
            .try_reserve_exact(count)
            .map_err(|_| Resource::Allocation)?;
        budget.reserve_storage(
            definitions
                .capacity()
                .checked_sub(count)
                .and_then(|n| n.checked_mul(width))
                .ok_or(Resource::Arithmetic)?,
        )?;
        budget.charge_work(count.checked_add(1).ok_or(Resource::Arithmetic)?)?;
        definitions.extend_from_slice(terminals.definitions());
        let signature = if let Some(call) = terminals.first_call() {
            // Capture the actual immutable contract immediately after trusted
            // CallOp::new built this sealed zero-argument/result occurrence.
            // Never clone a FunctionType vector or re-intern a temporary Box.
            budget.charge_work(count)?;
            let native = definitions
                .iter()
                .position(|coordinate| *coordinate == call.block.function)
                .ok_or(Failure::NativeSchema)?;
            budget.charge_work(checked_bridge_add_v12(
                4,
                checked_bridge_add_v12(
                    native,
                    checked_bridge_add_v12(call.block.block as usize, call.operation as usize)?,
                )?,
            )?)?;
            let region = root.deref(context).get_region(0);
            let block = region
                .deref(context)
                .iter(context)
                .next()
                .ok_or(Failure::NativeSchema)?;
            let function = block
                .deref(context)
                .iter(context)
                .nth(native)
                .ok_or(Failure::NativeSchema)?;
            let region = function.deref(context).get_region(0);
            let block = region
                .deref(context)
                .iter(context)
                .nth(call.block.block as usize)
                .ok_or(Failure::NativeSchema)?;
            let pointer = block
                .deref(context)
                .iter(context)
                .nth(call.operation as usize)
                .ok_or(Failure::NativeSchema)?;
            let native =
                Operation::get_op::<CallOp>(pointer, context).ok_or(Failure::NativeSchema)?;
            let raw = pointer.deref(context);
            if raw.get_num_operands() != 0 || raw.get_num_results() != 0 {
                return Err(Failure::NativeSchema);
            }
            Some(native.signature(context).ok_or(Failure::NativeSchema)?)
        } else {
            None
        };
        Ok(Some(Self {
            definitions,
            signature,
        }))
    }
    pub(super) fn len(&self) -> usize {
        self.definitions.len()
    }
    pub(super) fn module_ordinal(&self, native: usize) -> Option<usize> {
        self.definitions
            .get(native)
            .map(|coordinate| coordinate.0 as usize)
    }
    pub(super) fn native_ordinal(
        &self,
        module: usize,
        budget: &mut Budget<'_>,
    ) -> Result<usize, Failure> {
        budget.charge_work(self.definitions.len())?;
        self.definitions
            .iter()
            .position(|coordinate| coordinate.0 as usize == module)
            .ok_or(Failure::InvalidQuery { function: module })
    }
    pub(super) fn signature(&self) -> Option<TypeHandle> {
        self.signature
    }
}

pub(super) fn terminal(context: &Context, pointer: Ptr<Operation>) -> bool {
    use pliron::builtin::{
        attributes::OperandSegmentSizesAttr, op_interfaces::ATTR_KEY_OPERAND_SEGMENT_SIZES,
    };
    let Some(op) = Operation::get_op::<PreservedTerminatorOp>(pointer, context) else {
        return false;
    };
    let raw = pointer.deref(context);
    op.kind(context) == Some(PreservedTerminatorKindAttr::Unreachable)
        && raw.get_num_operands() == 0
        && raw.get_num_results() == 0
        && raw.get_num_successors() == 0
        && raw.num_regions() == 0
        && raw
            .attributes
            .get::<OperandSegmentSizesAttr>(&ATTR_KEY_OPERAND_SEGMENT_SIZES)
            .is_some_and(|sizes| sizes.0.is_empty())
}

pub(super) fn keys(context: &Context, pointer: Ptr<Operation>) -> Option<&'static [&'static str]> {
    terminal(context, pointer)
        .then_some(&["gpu_preserved_terminator_kind", "operand_segment_sizes"])
}

pub(super) fn attribute(key: &str, dialect: &str, name: &str) -> bool {
    dialect == "gpu"
        && key == "gpu_preserved_terminator_kind"
        && name == "preserved_terminator_kind"
}
