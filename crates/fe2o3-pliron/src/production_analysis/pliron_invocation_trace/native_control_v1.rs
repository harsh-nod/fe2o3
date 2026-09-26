//! Selector truth and simultaneous binding over the existing borrowed edge view.
use super::native_events_v1::NativeEventKindV1;
use super::native_input_v1::{NativeTraceRefusalV1, failure, native_resource};
use super::native_values_v1::NativeTraceStateV1;
use super::*;
use crate::production_analysis::pliron_control_edges_v1::ControlViewV1;
use dialect_gpu::{
    optimization_v1::{BranchOp as NativeBranch, CondBranchOp, ReturnOp as NativeReturn},
    switch_v3::SwitchOpV3,
};

impl NativeTraceStateV1<'_, '_, '_, '_> {
    pub(crate) fn control(
        &mut self,
        context: &Context,
        pointer: Ptr<Operation>,
        block: usize,
        events: &mut Vec<PlironTraceEventV1>,
        total: &mut usize,
    ) -> Result<Option<usize>, PlironTraceFailureV1> {
        self.budget.charge_work(1).map_err(native_resource)?;
        let input = self.input;
        let ordinal = *input
            .function
            .occurrence_index
            .get(&pointer)
            .ok_or_else(|| failure(block, 0, NativeTraceRefusalV1::Correspondence))?;
        let occurrence = &input.function.occurrences[ordinal];
        let bad = || {
            failure(
                block,
                occurrence.operation,
                NativeTraceRefusalV1::Correspondence,
            )
        };
        let control = ControlViewV1::observe(context, pointer).map_err(|_| bad())?;
        if Operation::is_op::<NativeReturn>(pointer, context) {
            self.budget
                .charge_work(pointer.deref(context).get_num_operands())
                .map_err(native_resource)?;
            self.push_event(
                events,
                PlironTraceEventV1::NativeSubject {
                    location: PlironTraceLocationV1 {
                        block,
                        operation: occurrence.operation,
                    },
                    occurrence: ordinal,
                    kind: NativeEventKindV1::Return,
                    address: None,
                },
            )?;
            return Ok(None);
        }
        let edge = if Operation::is_op::<NativeBranch>(pointer, context) {
            0
        } else if Operation::is_op::<CondBranchOp>(pointer, context) {
            let selector = self
                .evaluate(context, pointer.deref(context).get_operand(0), total)?
                .ok_or(PlironTraceFailureV1::UnresolvedBranch { block })?;
            if selector.width != 1 {
                return Err(bad());
            }
            usize::from(selector.bits == 0)
        } else if let Some(switch) = Operation::get_op::<SwitchOpV3>(pointer, context) {
            let selector = self
                .evaluate(context, switch.selector(context).ok_or_else(bad)?, total)?
                .ok_or(PlironTraceFailureV1::UnresolvedBranch { block })?;
            let cases = switch.cases(context).ok_or_else(bad)?;
            let mut selected = cases.bits().len();
            for (ordinal, bits) in cases.bits().iter().enumerate() {
                self.budget.charge_work(1).map_err(native_resource)?;
                if u128::from(*bits) == selector.bits {
                    selected = ordinal;
                    break;
                }
            }
            selected
        } else {
            return Err(failure(
                block,
                occurrence.operation,
                NativeTraceRefusalV1::UnsupportedTermination,
            ));
        };
        let edge = control.edge(edge).map_err(|_| bad())?;
        let next = *input
            .function
            .block_index
            .get(&edge.target())
            .ok_or_else(bad)?;
        let floor = self.budget.storage();
        let mut bindings =
            super::native_resources_v1::reserve_rows(edge.argument_count(), self.budget)
                .map_err(super::native_values_v1::failure_from_resource)?;
        for slot in 0..edge.argument_count() {
            self.budget.charge_work(1).map_err(native_resource)?;
            let (value, argument) = edge.argument_at(slot).map_err(|_| bad())?;
            let value = self.evaluate(context, value, total)?;
            bindings.push((argument, value));
        }
        for (argument, value) in &bindings {
            self.budget.charge_work(1).map_err(native_resource)?;
            if let Some(value) = value {
                self.environment.insert(*argument, *value);
            } else {
                self.environment.remove(argument);
            }
        }
        drop(bindings);
        self.budget
            .release_storage(self.budget.storage() - floor)
            .map_err(native_resource)?;
        Ok(Some(next))
    }
}
