//! Inert terminal observations from the collector's single authenticated walk.
use crate::production_semantic_terminal_v1::{
    ProductionSemanticTerminalRuleV1 as Rule, ProductionTerminalExpansionV1 as Expansion,
};
use crate::rustc_semantic_plan_v1::SourceClosureWorkV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CollectedTileTerminalKindV259 {
    MaskedLoad,
    IntoFragment,
    IntoParts,
}

impl CollectedTileTerminalKindV259 {
    const fn index(self) -> usize {
        match self {
            Self::MaskedLoad => 0,
            Self::IntoFragment => 1,
            Self::IntoParts => 2,
        }
    }
}

/// Counts call occurrences in unique collected monomorphized bodies. A shared
/// helper is not recounted per root, call context, or dynamic invocation. These
/// counts need not equal the later imported terminal-expansion roster.
///
/// Only authenticated terminal-stop handling in the collector constructs this
/// private inline state. Its borrowed queries choose no importer and establish
/// no source refinement, expansion support, proof, or publication authority.
pub(crate) struct CollectedTileTerminalCensusV259 {
    calls: [u64; 3],
}

impl CollectedTileTerminalCensusV259 {
    /// Conservative fixed inline owner and returned-borrow header charge. Each
    /// query leaves its charge on the supplied caller account, without refund.
    /// This observation does not authenticate that account as execution custody.
    pub(crate) const QUERY_STORAGE: usize = size_of::<Self>() + size_of::<&Self>();
    pub(crate) const QUERY_WORK: usize = Self::QUERY_STORAGE + 1;

    pub(super) const fn empty() -> Self {
        Self { calls: [0; 3] }
    }

    pub(super) fn record(
        &mut self,
        rule: Rule,
        work: &mut SourceClosureWorkV1,
    ) -> Result<(), super::CollectError> {
        let charge = |work: &mut SourceClosureWorkV1| {
            work.charge(1).map_err(|error| super::CollectError {
                message: format!("collected tile terminal census: {error}"),
            })
        };
        charge(work)?;
        let kind = match rule {
            Rule::Expand(Expansion::MaskedTileLoadU32) => CollectedTileTerminalKindV259::MaskedLoad,
            Rule::Expand(Expansion::MaskedTileIntoFragmentU32) => {
                CollectedTileTerminalKindV259::IntoFragment
            }
            Rule::Expand(Expansion::LaneFragmentIntoPartsU32) => {
                CollectedTileTerminalKindV259::IntoParts
            }
            _ => return Ok(()),
        };
        charge(work)?;
        let count = &mut self.calls[kind.index()];
        *count = count.checked_add(1).ok_or_else(|| super::CollectError {
            message: "collected tile terminal call count overflowed".to_owned(),
        })?;
        Ok(())
    }

    pub(super) fn borrow_on_account(&self, budget: &mut Budget<'_>) -> Result<&Self, Resource> {
        budget.check_prior_denials_v1()?;
        budget.charge_work(Self::QUERY_WORK)?;
        budget.reserve_storage(Self::QUERY_STORAGE)?;
        Ok(self)
    }

    pub(crate) const fn has_tile_operations(&self) -> bool {
        self.calls[0] != 0 || self.calls[1] != 0 || self.calls[2] != 0
    }

    pub(crate) const fn call_occurrences(&self, kind: CollectedTileTerminalKindV259) -> u64 {
        self.calls[kind.index()]
    }
}

#[cfg(test)]
#[path = "terminal_census_v259_tests.rs"]
mod tests;
