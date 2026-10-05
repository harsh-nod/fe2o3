//! Adopts the shared original SSA equations inside the bounded proof generator.
//! Actual source topology and emitted control-flow joins remain the caller's duty.

use super::{Error, Result, Writer};
use fe2o3_kernel_analysis::{
    SourceSsaBoundariesV31 as Checked, SourceSsaBoundaryErrorV31 as BoundaryError,
    SourceSsaBoundaryStorageV31 as Storage,
};
use fe2o3_mir_model::{
    SsaBlockIdV1 as Block, SsaConstructionPlanV1 as Plan, SsaValueV1 as Value,
    SsaVariableIdV1 as Variable,
};
use std::mem::size_of;

pub(super) struct ControlInput<'a> {
    pub entry: Block,
    pub successors: &'a [Vec<Block>],
}

pub(super) struct Boundaries<'a> {
    plan: &'a Plan,
    checked: Checked<'a>,
}

fn error(error: BoundaryError) -> Error {
    match error {
        BoundaryError::Resource(resource) => Error::Resource(resource),
        BoundaryError::Statement(detail) => Error::Statement(detail),
        BoundaryError::ForeignPlan => {
            Error::Statement("original MIR SSA boundary has a foreign plan")
        }
        BoundaryError::Panicked => {
            Error::Statement("original MIR SSA boundary derivation panicked")
        }
    }
}

fn headers() -> usize {
    size_of::<Boundaries<'_>>()
        + size_of::<Result<Boundaries<'_>>>()
        + size_of::<ControlInput<'_>>()
        + size_of::<Storage>()
        + size_of::<std::result::Result<(Checked<'_>, Storage), BoundaryError>>()
        + size_of::<std::result::Result<Value, BoundaryError>>()
        + size_of::<Result<Value>>()
        + size_of::<Result<()>>()
        + size_of::<&Plan>()
        + size_of::<&mut Writer<'_, '_>>()
        + size_of::<Block>()
        + size_of::<Variable>()
}

impl<'a> Boundaries<'a> {
    pub(super) fn derive(
        plan: &'a Plan,
        input: ControlInput<'a>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        let (checked, storage) =
            Checked::derive(plan, input.entry, input.successors, out.budget).map_err(error)?;
        // The shared checker restores its scratch floor and returns an
        // unreserved owner. Its actual retained capacity is adopted here.
        out.budget.reserve_storage(storage.retained_storage())?;
        Ok(Self { plan, checked })
    }

    pub(super) fn value(
        &self,
        block: Block,
        variable: Variable,
        out: &mut Writer<'_, '_>,
    ) -> Result<Value> {
        self.checked
            .value(self.plan, block, variable, out.budget)
            .map_err(error)
    }
}

#[cfg(test)]
#[path = "original_semantic_mir_boundary_v31_tests.rs"]
mod tests;
