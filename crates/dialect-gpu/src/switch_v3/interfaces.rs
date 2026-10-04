use super::*;

#[cfg(test)]
pub(super) mod instrumentation {
    std::thread_local! {
        pub(in crate::switch_v3) static PAYLOAD_COPIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }
}

#[op_interface_impl]
impl OperandSegmentInterface for SwitchOpV3 {
    fn verify(op: &dyn Op, ctx: &Context) -> Result<()> {
        let raw = op.get_operation().deref(ctx);
        let Some(sizes) = raw
            .attributes
            .get::<OperandSegmentSizesAttr>(&ATTR_KEY_OPERAND_SEGMENT_SIZES)
        else {
            return verify_err!(
                op.loc(ctx),
                "native switch requires generated operand segments"
            );
        };
        // The default interface sums untrusted u32 sizes without checking overflow.
        if sizes.0.len() != 2
            || sizes.0[0] != 1
            || sizes.0[1]
                .checked_add(1)
                .and_then(|total| usize::try_from(total).ok())
                != Some(raw.get_num_operands())
        {
            return verify_err!(
                op.loc(ctx),
                "native switch operand segments must be exactly [1, payload]"
            );
        }
        Ok(())
    }
}

#[op_interface_impl]
impl BranchOpInterface for SwitchOpV3 {
    fn verify(op: &dyn Op, ctx: &Context) -> Result<()> {
        let Some(switch) = Operation::get_op::<Self>(op.get_operation(), ctx) else {
            return verify_err!(
                op.loc(ctx),
                "native switch interface requires a switch operation"
            );
        };
        switch.verify_successor_signatures(ctx)
    }

    fn successor_operands(&self, ctx: &Context, ordinal: usize) -> Vec<Value> {
        #[cfg(test)]
        instrumentation::PAYLOAD_COPIES.with(|count| count.set(count.get() + 1));
        // Generated interface verification can run before custom verification.
        let Some(range) = self.successor_operand_range(ctx, ordinal) else {
            return Vec::new();
        };
        let raw = self.get_operation().deref(ctx);
        range.map(|index| raw.get_operand(index)).collect()
    }

    fn add_successor_operand(&self, ctx: &mut Context, ordinal: usize, value: Value) -> usize {
        let range = self
            .successor_operand_range(ctx, ordinal)
            .expect("valid switch successor ordinal");
        assert!(
            range.len() < MAX_SWITCH_EDGE_ARGUMENTS_V3,
            "switch edge wire ceiling"
        );
        let mut offsets = self
            .get_attr_gpu_switch_offsets(ctx)
            .expect("switch offsets")
            .0
            .clone();
        let length = self.get_operation().deref(ctx).get_num_operands();
        assert!(
            u32::try_from(length).is_ok_and(|length| length < u32::MAX),
            "switch operand ceiling"
        );
        for offset in &mut offsets[ordinal + 1..] {
            *offset = offset
                .checked_add(1)
                .expect("checked switch operand ceiling");
        }
        self.insert_into_segment(ctx, 1, range.end - 1, value);
        self.set_attr_gpu_switch_offsets(ctx, SwitchSuccessorOffsetsAttrV3(offsets));
        range.len()
    }

    fn remove_successor_operand(
        &self,
        ctx: &mut Context,
        ordinal: usize,
        argument: usize,
    ) -> Value {
        let range = self
            .successor_operand_range(ctx, ordinal)
            .expect("valid switch successor ordinal");
        assert!(argument < range.len(), "valid switch successor argument");
        let mut offsets = self
            .get_attr_gpu_switch_offsets(ctx)
            .expect("switch offsets")
            .0
            .clone();
        for offset in &mut offsets[ordinal + 1..] {
            *offset = offset
                .checked_sub(1)
                .expect("nonempty switch argument suffix");
        }
        let value = self.remove_from_segment(ctx, 1, range.start - 1 + argument);
        self.set_attr_gpu_switch_offsets(ctx, SwitchSuccessorOffsetsAttrV3(offsets));
        value
    }
}
