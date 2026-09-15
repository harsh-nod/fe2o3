//! Exact-event phase emission and replay. Request data is inert; the production
//! compiler must retain its private live source owner while invoking this path.
use super::*;
use fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedDefinedCapabilityV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticDefinedCapabilityContractV1 as Defined, SemanticDefinedReusablePhaseRecipeV1 as Recipe,
};
use fe2o3_mir_model::{SemanticCallInstanceIdV1, SsaBlockIdV1, SsaResolvedEventV1};
use fe2o3_pliron::ProductionSemanticSsaSourceQueryV1;

mod borrow_sites;
mod calls;
mod constructors;
mod descriptors;
mod epochs;
pub(super) use epochs::FinishEpoch;
mod input;
mod rows;
mod runtime;
mod schedule;
mod scope;
pub use input::*;
use rows::CheckedRows;
pub(super) use runtime::Runtime;
pub(super) use scope::{PhaseReplayV1, replay, with_replay_work};
pub use scope::{ProductionSemanticPhaseEmissionResultV1, ProductionSemanticPhaseEmissionScopeV1};

type PhaseResult<T> = Result<T, ProductionSemanticKirErrorV1>;

fn rejected(detail: &'static str) -> ProductionSemanticKirErrorV1 {
    unsupported(0, None, None, detail)
}

fn spend(work: &mut usize, count: usize) -> PhaseResult<()> {
    *work = work
        .checked_sub(count)
        .ok_or_else(|| rejected("phase emission exhausted existing shared work"))?;
    Ok(())
}

fn reserve<T>(count: usize, work: &mut usize) -> PhaseResult<Vec<T>> {
    let requested = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| rejected("phase emission capacity overflow"))?;
    spend(work, requested)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| rejected("phase emission allocation failed"))?;
    let actual = values
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| rejected("phase emission retained capacity overflow"))?;
    spend(
        work,
        actual
            .checked_sub(requested)
            .ok_or_else(|| rejected("phase emission capacity is smaller than requested"))?,
    )?;
    Ok(values)
}
