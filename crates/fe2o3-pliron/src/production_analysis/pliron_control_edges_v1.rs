//! Borrowed edge shapes from the authenticated, immutable production graph.
//!
//! This is not structural admission or a second CFG. Native verification owns
//! operation semantics, key validity and type correctness. Callers budget their
//! edge/payload visits and any value-type lookup; observation never walks keys
//! or payloads, resolves value types, or allocates a successor-operand vector.

use std::{cell::Ref, ops::Range};

use dialect_gpu::{
    optimization_v1::{BranchOp as GpuBranchOp, CondBranchOp, ReturnOp as GpuReturnOp},
    switch_v3::{
        MAX_SWITCH_CASES_V3, MAX_SWITCH_EDGE_ARGUMENTS_V3, SwitchOpV3, SwitchSuccessorOffsetsAttrV3,
    },
};
use dialect_kernel::{
    AnalysisSplitOp, BranchArgsOp, BranchOp, IndexEqualBranchArgsOp, IndexEqualBranchOp,
    IndexLessThanBranchArgsOp, IndexLessThanBranchOp, ReturnOp, TrapOp,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{attributes::OperandSegmentSizesAttr, op_interfaces::ATTR_KEY_OPERAND_SEGMENT_SIZES},
    context::{Context, Ptr},
    operation::Operation,
    value::Value,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ControlErrorV1 {
    Unsupported,
    Shape,
    OperandCount,
    MissingEdgeArguments,
    EdgeOrdinal,
    ArgumentOrdinal,
}

enum LayoutV1<'ctx> {
    Exit,
    Single,
    Binary {
        controls: usize,
        first_end: usize,
        less_than: bool,
    },
    Switch(Ref<'ctx, SwitchSuccessorOffsetsAttrV3>),
}

pub(crate) struct ControlViewV1<'ctx> {
    context: &'ctx Context,
    raw: Ref<'ctx, Operation>,
    layout: LayoutV1<'ctx>,
}

impl<'ctx> ControlViewV1<'ctx> {
    pub(crate) fn observe(
        context: &'ctx Context,
        pointer: Ptr<Operation>,
    ) -> Result<Self, ControlErrorV1> {
        let raw = pointer.deref(context);
        if raw.get_num_results() != 0 || raw.num_regions() != 0 {
            return Err(ControlErrorV1::Shape);
        }
        let layout = if let Some(switch) = Operation::get_op::<SwitchOpV3>(pointer, context) {
            let cases = switch.cases(context).ok_or(ControlErrorV1::Shape)?;
            let offsets = switch
                .get_attr_gpu_switch_offsets(context)
                .ok_or(ControlErrorV1::Shape)?;
            let segments = raw
                .attributes
                .get::<OperandSegmentSizesAttr>(&ATTR_KEY_OPERAND_SEGMENT_SIZES)
                .ok_or(ControlErrorV1::Shape)?;
            let payloads = raw
                .get_num_operands()
                .checked_sub(1)
                .ok_or(ControlErrorV1::Shape)?;
            if raw.attributes.0.len() != 4
                || switch.kind(context).is_none()
                || cases.bits().len() > MAX_SWITCH_CASES_V3
                || raw.get_num_successors() != cases.bits().len() + 1
                || offsets.offsets().len() != raw.get_num_successors() + 1
                || offsets.offsets().first() != Some(&0)
                || offsets.offsets().last().copied().map(|n| n as usize) != Some(payloads)
                || segments.0.len() != 2
                || segments.0[0] != 1
                || segments.0[1] as usize != payloads
                || segments.0[1].checked_add(1).is_none()
            {
                return Err(ControlErrorV1::Shape);
            }
            LayoutV1::Switch(offsets)
        } else if Operation::get_op::<BranchOp>(pointer, context).is_some() {
            if raw.get_num_operands() != 0 || raw.get_num_successors() != 1 {
                return Err(ControlErrorV1::Shape);
            }
            if raw.get_successor(0).deref(context).get_num_arguments() != 0 {
                return Err(ControlErrorV1::MissingEdgeArguments);
            }
            LayoutV1::Single
        } else if Operation::get_op::<BranchArgsOp>(pointer, context).is_some()
            || Operation::get_op::<GpuBranchOp>(pointer, context).is_some()
        {
            if raw.get_num_successors() != 1 {
                return Err(ControlErrorV1::Shape);
            }
            LayoutV1::Single
        } else if Operation::get_op::<IndexLessThanBranchOp>(pointer, context).is_some()
            || Operation::get_op::<IndexEqualBranchOp>(pointer, context).is_some()
        {
            if raw.get_num_successors() != 2 || raw.get_num_operands() != 2 {
                return Err(ControlErrorV1::Shape);
            }
            LayoutV1::Binary {
                controls: 2,
                first_end: 2,
                less_than: Operation::get_op::<IndexLessThanBranchOp>(pointer, context).is_some(),
            }
        } else if Operation::get_op::<IndexLessThanBranchArgsOp>(pointer, context).is_some()
            || Operation::get_op::<IndexEqualBranchArgsOp>(pointer, context).is_some()
        {
            Self::binary_layout(
                context,
                &raw,
                2,
                Operation::get_op::<IndexLessThanBranchArgsOp>(pointer, context).is_some(),
            )?
        } else if Operation::get_op::<CondBranchOp>(pointer, context).is_some() {
            let segments = raw
                .attributes
                .get::<OperandSegmentSizesAttr>(&ATTR_KEY_OPERAND_SEGMENT_SIZES)
                .ok_or(ControlErrorV1::Shape)?;
            if raw.get_num_successors() != 2 || segments.0.len() != 3 || segments.0[0] != 1 {
                return Err(ControlErrorV1::Shape);
            }
            let first_end = 1_usize
                .checked_add(segments.0[1] as usize)
                .ok_or(ControlErrorV1::Shape)?;
            if first_end.checked_add(segments.0[2] as usize) != Some(raw.get_num_operands()) {
                return Err(ControlErrorV1::OperandCount);
            }
            LayoutV1::Binary {
                controls: 1,
                first_end,
                less_than: false,
            }
        } else if let Some(split) = Operation::get_op::<AnalysisSplitOp>(pointer, context) {
            let controls = split
                .get_attr_kernel_analysis_split_control_count(context)
                .ok_or(ControlErrorV1::Shape)?
                .count() as usize;
            Self::binary_layout(context, &raw, controls, false)?
        } else if Operation::get_op::<ReturnOp>(pointer, context).is_some()
            || Operation::get_op::<TrapOp>(pointer, context).is_some()
            || Operation::get_op::<GpuReturnOp>(pointer, context).is_some()
        {
            if raw.get_num_successors() != 0
                || (Operation::get_op::<GpuReturnOp>(pointer, context).is_none()
                    && raw.get_num_operands() != 0)
            {
                return Err(ControlErrorV1::Shape);
            }
            LayoutV1::Exit
        } else {
            return Err(ControlErrorV1::Unsupported);
        };
        Ok(Self {
            context,
            raw,
            layout,
        })
    }

    fn binary_layout(
        context: &Context,
        raw: &Operation,
        controls: usize,
        less_than: bool,
    ) -> Result<LayoutV1<'ctx>, ControlErrorV1> {
        if raw.get_num_successors() != 2 {
            return Err(ControlErrorV1::Shape);
        }
        let first_end = controls
            .checked_add(raw.get_successor(0).deref(context).get_num_arguments())
            .ok_or(ControlErrorV1::Shape)?;
        if first_end.checked_add(raw.get_successor(1).deref(context).get_num_arguments())
            != Some(raw.get_num_operands())
        {
            return Err(ControlErrorV1::OperandCount);
        }
        Ok(LayoutV1::Binary {
            controls,
            first_end,
            less_than,
        })
    }

    pub(crate) fn successor_count(&self) -> usize {
        self.raw.get_num_successors()
    }

    pub(crate) fn edge(&self, ordinal: usize) -> Result<EdgeViewV1<'_, 'ctx>, ControlErrorV1> {
        if ordinal >= self.successor_count() {
            return Err(ControlErrorV1::EdgeOrdinal);
        }
        let range = match &self.layout {
            LayoutV1::Exit => return Err(ControlErrorV1::EdgeOrdinal),
            LayoutV1::Single => 0..self.raw.get_num_operands(),
            LayoutV1::Binary {
                controls,
                first_end,
                ..
            } => {
                if ordinal == 0 {
                    *controls..*first_end
                } else {
                    *first_end..self.raw.get_num_operands()
                }
            }
            LayoutV1::Switch(offsets) => {
                let start = offsets.offsets()[ordinal] as usize;
                let end = offsets.offsets()[ordinal + 1] as usize;
                if end
                    .checked_sub(start)
                    .is_none_or(|n| n > MAX_SWITCH_EDGE_ARGUMENTS_V3)
                {
                    return Err(ControlErrorV1::Shape);
                }
                start.checked_add(1).ok_or(ControlErrorV1::Shape)?
                    ..end.checked_add(1).ok_or(ControlErrorV1::Shape)?
            }
        };
        let target = self.raw.get_successor(ordinal);
        if range.end > self.raw.get_num_operands()
            || range.end.checked_sub(range.start)
                != Some(target.deref(self.context).get_num_arguments())
        {
            return Err(ControlErrorV1::Shape);
        }
        Ok(EdgeViewV1 {
            control: self,
            ordinal,
            target,
            range,
        })
    }
}

pub(crate) struct EdgeViewV1<'view, 'ctx> {
    control: &'view ControlViewV1<'ctx>,
    ordinal: usize,
    target: Ptr<BasicBlock>,
    range: Range<usize>,
}

impl EdgeViewV1<'_, '_> {
    pub(crate) fn target(&self) -> Ptr<BasicBlock> {
        self.target
    }

    pub(crate) fn argument_count(&self) -> usize {
        self.range.len()
    }

    /// Exact ordered values, without an implicit (potentially linear) type query.
    pub(crate) fn argument_at(&self, index: usize) -> Result<(Value, Value), ControlErrorV1> {
        if index >= self.range.len() {
            return Err(ControlErrorV1::ArgumentOrdinal);
        }
        Ok((
            self.control.raw.get_operand(self.range.start + index),
            self.target.deref(self.control.context).get_argument(index),
        ))
    }

    pub(crate) fn index_less_than_guard(&self) -> Option<(Value, Value)> {
        if self.ordinal == 0
            && matches!(
                self.control.layout,
                LayoutV1::Binary {
                    less_than: true,
                    ..
                }
            )
        {
            Some((
                self.control.raw.get_operand(0),
                self.control.raw.get_operand(1),
            ))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests;
