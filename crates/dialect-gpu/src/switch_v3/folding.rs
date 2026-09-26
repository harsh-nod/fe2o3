use super::*;
use crate::optimization_v1::{BranchOp, IndexAttr};
use pliron::{
    attribute::AttrObj,
    builtin::attributes::IntegerAttr,
    irbuild::{IRStatus, observer::RewriteOccurrenceEvent, rewriter::Rewriter},
    opts::constants::BranchOpFoldInterface,
};

impl SwitchOpV3 {
    pub(super) fn selected_successor(
        &self,
        ctx: &Context,
        operands: &[Option<AttrObj>],
    ) -> Option<usize> {
        let keys = self.cases(ctx)?;
        let kind = self.kind(ctx)?;
        let selector = self.selector(ctx)?;
        let (width, signed, index) = attributes::selector_shape(ctx, selector.get_type(ctx))?;
        if keys.bits().is_empty() {
            return Some(0);
        }
        let constant = operands.first()?.as_ref()?;
        let bits = if index {
            u128::from(constant.downcast_ref::<IndexAttr>()?.0)
        } else {
            let integer = constant.downcast_ref::<IntegerAttr>()?;
            let ty: TypeHandle = integer.get_type().into();
            if ty != selector.get_type(ctx) || integer.value().bw() != width as usize {
                return None;
            }
            // Conversion is exact for every admitted selector width, including
            // legacy 128-bit selectors whose high bits must not be discarded.
            integer.value().to_u128()
        };
        let Ok(bits) = u64::try_from(bits) else { return Some(keys.bits().len()); };
        let selected = if kind == SwitchKeyKindAttrV3::LegacyU64 {
            keys.bits().iter().position(|key| *key == bits)
        } else {
            if width > 64 { return None; }
            let sign = if signed { 1_u64 << (width - 1) } else { 0 };
            keys.bits().binary_search_by_key(&(bits ^ sign), |key| *key ^ sign).ok()
        };
        Some(selected.unwrap_or(keys.bits().len()))
    }
}

#[op_interface_impl]
impl BranchOpFoldInterface for SwitchOpV3 {
    fn check_fold(&self, ctx: &Context, operands: &[Option<AttrObj>]) -> Vec<Ptr<BasicBlock>> {
        let raw = self.get_operation().deref(ctx);
        match self.selected_successor(ctx, operands) {
            Some(ordinal) => vec![raw.get_successor(ordinal)],
            None => raw.successors().collect(),
        }
    }

    fn fold_in_place(
        &self,
        ctx: &mut Context,
        operands: &[Option<AttrObj>],
        rewriter: &mut dyn Rewriter,
    ) -> IRStatus {
        let Some(successor) = self.selected_successor(ctx, operands) else {
            return IRStatus::Unchanged;
        };
        let destination = self.get_operation().deref(ctx).get_successor(successor);
        let replacement = BranchOp::new(ctx, destination, self.successor_operands(ctx, successor))
            .get_operation();
        rewriter.insert_operation(ctx, replacement);
        if rewriter.observes_occurrences() {
            rewriter.notify_occurrence(ctx, RewriteOccurrenceEvent::BranchSuccessorSelected {
                old: self.get_operation(), new: replacement, successor,
            });
        }
        rewriter.replace_operation(ctx, self.get_operation(), replacement);
        IRStatus::Changed
    }
}
