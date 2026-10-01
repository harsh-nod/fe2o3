//! One retained function-plan owner, not the whole semantic SSA pipeline.
use super::ProductionSemanticSsaFunctionPlanV1;
use fe2o3_kernel_ir::{LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error};
use std::mem::size_of;

fn fixed<T: Copy>(_: &T) {}

fn charge_extent(counter: &mut Counter, count: usize, width: usize) -> Result<(), Error> {
    let bytes = count.checked_mul(width).ok_or(Error::Arithmetic)?;
    counter.charge(bytes, 1)
}

// Keep the Box shape explicit: a Vec substitution must not count only its length.
#[allow(clippy::borrowed_box)]
fn boxed<T: Copy>(values: &Box<[T]>, counter: &mut Counter) -> Result<(), Error> {
    charge_extent(counter, values.len(), size_of::<T>())
}

impl ProductionSemanticSsaFunctionPlanV1 {
    /// Charges one complete function-plan header and its actual owned heap.
    ///
    /// The header includes all embedded child headers and fixed metadata once.
    /// The exact retained planner output and both boxed variable arrays are
    /// observed without reconstruction. Stored storage/work estimates are not
    /// added. The caller supplies the shared byte/item limits and must discard
    /// the whole observation on any error; this counter may then be partial.
    ///
    /// This is not the full SSA owner: semantic MIR/Pliron context, the plans
    /// collection itself, optional occurrence capture and temporary/peak/RSS
    /// storage are outside this single function-plan root. Does not replay or
    /// change any constructor, admission envelope, canonical identity or policy.
    pub fn charge_retained_storage_v1(&self, counter: &mut Counter) -> Result<(), Error> {
        counter.charge(size_of::<Self>(), 1)?;
        self.charge_retained_heap_storage_v1(counter)
    }

    /// Charges only this function plan's heap to a bounded enclosing ledger.
    ///
    /// Excludes this wrapper's inline header and root visit. The nested planner
    /// visitor includes its own root visit. Box payload lengths are exact;
    /// their inline headers already belong to the enclosing wrapper. No local
    /// unlimited ledger, preliminary scan or serialized-size substitute is used.
    /// First refusal propagates; the caller must discard a partial observation.
    pub fn charge_retained_heap_storage_v1(&self, counter: &mut Counter) -> Result<(), Error> {
        let Self {
            function,
            function_identity,
            plan,
            partial_moves,
            implicit_entry_variables,
            retained_cross_edge_variables,
            auxiliary_resources,
        } = self;
        fixed(function);
        fixed(function_identity);
        fixed(partial_moves);
        fixed(auxiliary_resources);
        plan.visit_logical_retained_heap_v1(&mut |count, width| {
            charge_extent(counter, count, width)
        })?;
        boxed(implicit_entry_variables, counter)?;
        boxed(retained_cross_edge_variables, counter)
    }
}

#[cfg(test)]
#[path = "function_plan_storage_v1_tests.rs"]
mod tests;
