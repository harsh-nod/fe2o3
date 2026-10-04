//! Inert retained source singleton census. No source, query or routing authority.
//! Original discovery/Census methods are reused; the old wrapper is untouched.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_preparation_resources_v1::{
    PreparationCustodySnapshotV1 as Snapshot, PreparationResourcesV1 as Prep,
};
use fe2o3_mir_model::semantic_mir_v1 as model;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBasicBlockV1, SemanticCallDestinationV1, SemanticLocalDeclV1, SemanticProjectionV1,
    SemanticStatementV1,
};
use std::mem::size_of;
type Error = ProductionRankedProjectionErrorV1;
type R<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Fresh,
    Terminal,
    Complete,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Source {
    function: usize,
    types: usize,
    type_count: usize,
}
fn source(types: &[SemanticTypeDeclV1], function: &SemanticFunctionDeclV1) -> Source {
    Source {
        function: function as *const SemanticFunctionDeclV1 as usize,
        types: types.as_ptr() as usize,
        type_count: types.len(),
    }
}
fn accounting() -> Error {
    resource(Resource::Accounting)
}
fn arithmetic() -> Error {
    resource(Resource::Arithmetic)
}
fn unavailable() -> Error {
    Error::Incomplete("singleton census adapter cannot answer semantic queries")
}
pub(in crate::production_ranked_projection_v1) struct RetainedScalarSingletonV1 {
    phase: Phase,
    source: Option<Source>,
    entry: Option<Snapshot>,
    held: Option<Snapshot>,
    flags: Vec<u8>,
    scan_invoked: bool,
    explicit: bool,
}
impl RetainedScalarSingletonV1 {
    pub(in crate::production_ranked_projection_v1) fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            source: None,
            entry: None,
            held: None,
            flags: Vec::new(),
            scan_invoked: false,
            explicit: false,
        }
    }
    pub(in crate::production_ranked_projection_v1) fn prepare_into(
        &mut self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        resources: &mut Prep<'_, '_>,
    ) -> R<()> {
        let fresh = self.phase == Phase::Fresh;
        self.phase = Phase::Terminal;
        let entry = resources
            .retained_custody_snapshot_v1()
            .ok_or_else(accounting)?;
        if !fresh || entry.denied_work || entry.denied_storage || entry.owned > entry.storage {
            return Err(accounting());
        }
        self.entry = Some(entry);
        self.source = Some(source(types, function));
        resources.work(32)?;
        resources.reserve_storage(frame()?)?;
        self.prepare_original(types, function, resources)?;
        self.check(types, function, resources)?;
        self.held = Some(
            resources
                .retained_custody_snapshot_v1()
                .ok_or_else(accounting)?,
        );
        self.phase = Phase::Complete;
        Ok(())
    }
    fn prepare_original(
        &mut self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        resources: &mut Prep<'_, '_>,
    ) -> R<()> {
        let mut facts = Meter(resources);
        charge(&mut facts, 4)?;
        self.scan_invoked = true;
        self.explicit = has_explicit_whole_place(function, &mut facts)?;
        if !self.explicit {
            return Ok(());
        }
        let count = function.locals().len();
        let requested = count
            .checked_add(std::mem::size_of::<Vec<u8>>())
            .ok_or_else(|| resource(Resource::Arithmetic))?;

        facts.0.reserve_storage(requested)?;
        self.flags
            .try_reserve_exact(count)
            .map_err(|_| resource(Resource::Allocation))?;
        let excess = self
            .flags
            .capacity()
            .checked_sub(count)
            .ok_or_else(|| resource(Resource::Accounting))?;
        facts.0.reserve_storage(excess)?;
        charge(
            &mut facts,
            count
                .checked_mul(5)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )?;
        for local in function.locals() {
            let ty = types
                .get(local.ty().index() as usize)
                .ok_or_else(malformed)?;
            let is_scalar = matches!(ty.shape(), SemanticTypeShapeV1::Scalar(_));
            self.flags
                .push(if !local.role().is_entry_argument() && is_scalar {
                    ELIGIBLE
                } else {
                    0
                });
        }
        let mut census = Census {
            types,
            function,
            flags: &mut self.flags,
            facts: &mut facts,
        };
        for block in function.blocks() {
            charge(census.facts, 1)?;
            for statement in block.statements() {
                census.statement(statement.kind())?;
            }
            census.terminator(block.terminator().kind())?;
        }
        Ok(())
    }
    fn check(
        &self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        resources: &Prep<'_, '_>,
    ) -> R<()> {
        let before = self.entry.ok_or_else(accounting)?;
        let now = resources
            .retained_custody_snapshot_v1()
            .ok_or_else(accounting)?;
        let growth = now.owned.checked_sub(before.owned).ok_or_else(accounting)?;
        let expected = before.storage.checked_add(growth).ok_or_else(arithmetic)?;
        if let Some(held) = self.held {
            if now.owned < held.owned
                || now.storage < held.storage
                || now.work < held.work
                || now.peak < held.peak
            {
                return Err(accounting());
            }
        }
        if self.source != Some(source(types, function))
            || before.budget_slot != now.budget_slot
            || before.work_ledger != now.work_ledger
            || before.owned_slot != now.owned_slot
            || now.storage != expected
            || now.work < before.work
            || now.peak < before.peak
            || now.denied_work
            || now.denied_storage
            || !self.scan_invoked
        {
            return Err(accounting());
        }
        Ok(())
    }
    pub(in crate::production_ranked_projection_v1) fn completed_for<'a>(
        &'a self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        resources: &Prep<'_, '_>,
    ) -> R<&'a [u8]> {
        if self.phase != Phase::Complete {
            return Err(accounting());
        }
        self.check(types, function, resources)?;
        if (self.explicit && self.flags.len() != function.locals().len())
            || (!self.explicit && !self.flags.is_empty())
        {
            return Err(accounting());
        }
        Ok(&self.flags)
    }
}
/// This adapter lends ONLY original metered work to unchanged discovery/Census.
/// Required semantic methods refuse; no source proof or real facts is fabricated.
struct Meter<'r, 'b, 'w>(&'r mut Prep<'b, 'w>);
impl ProjectedAssertionFactsV1 for Meter<'_, '_, '_> {
    fn charge_private_array_work(&mut self, amount: usize) -> R<()> {
        self.0.work(amount)
    }
    fn private_array_initializer_count(
        &mut self,
        _block: usize,
        _statement: usize,
    ) -> R<Option<u64>> {
        Err(unavailable())
    }
    fn is_materialized_block(&mut self, _block: usize) -> R<bool> {
        Err(unavailable())
    }
    fn condition(
        &mut self,
        _block: usize,
        _expected: bool,
        _successor: SemanticBlockIdV1,
    ) -> R<canonical_assertion_facts_v1::ProjectedAssertionConditionV1> {
        Err(unavailable())
    }
}
fn frame() -> R<usize> {
    let rows=[
        size_of::<RetainedScalarSingletonV1>(),
        size_of::<(Phase,Option<Source>,Option<Snapshot>,Option<Snapshot>,Vec<u8>,bool,bool)>(),
        size_of::<(&mut RetainedScalarSingletonV1,&RetainedScalarSingletonV1,
            &[SemanticTypeDeclV1],&SemanticFunctionDeclV1,&mut Prep<'static,'static>,
            &Prep<'static,'static>,Source,Snapshot,Option<Snapshot>,bool,
            *const SemanticFunctionDeclV1,*const SemanticTypeDeclV1,usize,usize,usize)>(),
        size_of::<(Meter<'static,'static,'static>,&mut Meter<'static,'static,'static>,
            &mut dyn ProjectedAssertionFactsV1,&mut Prep<'static,'static>,
            Census<'static,'static>,&mut Census<'static,'static>)>(),
        size_of::<(usize,usize,usize,Option<usize>,R<usize>,R<()>,R<bool>,
            std::result::Result<(), std::collections::TryReserveError>,
            canonical_assertion_facts_v1::CanonicalAssertionErrorV1,
            std::collections::TryReserveError,Error,Resource)>(),
        size_of::<(std::slice::Iter<'static,SemanticLocalDeclV1>,&SemanticLocalDeclV1,
            &SemanticTypeDeclV1,Option<&SemanticTypeDeclV1>,&SemanticTypeShapeV1,
            SemanticLocalRoleV1,SemanticLocalRoleV1,SemanticTypeIdV1,SemanticTypeIdV1,u32,usize,bool,u8)>(),
        size_of::<(std::slice::Iter<'static,SemanticBasicBlockV1>,&SemanticBasicBlockV1,
            std::slice::Iter<'static,SemanticStatementV1>,&SemanticStatementV1,
            &SemanticStatementKindV1,&SemanticTerminatorKindV1,
            &mut Vec<u8>,&mut [u8],&Vec<u8>,&[u8],R<&[u8]>)>(),
        size_of::<(Snapshot,Snapshot,Snapshot,Option<Snapshot>,Option<Snapshot>,Option<Source>,Source,
            usize,usize,Option<usize>,Option<usize>,bool,R<()>)>(),
        size_of::<(&Prep<'static,'static>,&crate::production_ranked_projection_v1::bf16_nominal_preparation_resources_v1::PreparationCustodySnapshotV1,
            &fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'static>,
            &usize,Snapshot,Option<Snapshot>,usize,usize,usize,usize,usize,usize,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,bool,bool)>(),
        size_of::<(&mut Meter<'static,'static,'static>,usize,usize,usize,bool,
            SemanticBlockIdV1,R<Option<u64>>,R<bool>,
            R<canonical_assertion_facts_v1::ProjectedAssertionConditionV1>,Error)>(),
        // Explicit lexical carriers reached inside the unchanged Census;
        // the old work/flag semantics are not replaced by this frame policy.
        size_of::<(&SemanticPlaceV1,&SemanticPlaceV1,&SemanticOperandV1,&SemanticRvalueV1,
            &SemanticStatementKindV1,&SemanticAssertMessageV1,&SemanticTerminatorKindV1,
            &SemanticLocalDeclV1,Option<&SemanticLocalDeclV1>,
            &SemanticTypeDeclV1,Option<&SemanticTypeDeclV1>,Option<&SemanticPlaceV1>,
            &mut u8,Option<&mut u8>,bool,bool,usize,Option<usize>,R<()>)>(),
        size_of::<(std::slice::Iter<'static,SemanticProjectionV1>,&SemanticProjectionV1,
            &SemanticProjectionKindV1,SemanticProjectionV1,SemanticProjectionV1,
            SemanticProjectionKindV1,SemanticTypeIdV1,SemanticTypeIdV1,
            SemanticLocalIdV1,SemanticLocalIdV1,SemanticLocalIdV1,SemanticLocalIdV1,u32,u32,usize,usize,
            std::slice::Iter<'static,SemanticOperandV1>,&SemanticOperandV1,
            &SemanticDirectCallV1,&SemanticCallDestinationV1,Option<&SemanticCallDestinationV1>)>(),
        // Actual borrowed enum-arm payloads and getter carriers in unchanged
        // discovery/Census, separate from the enclosing enum references.
        size_of::<(&model::SemanticConstantV1,&model::SemanticAssignmentV1,
            &model::SemanticMemoryStoreV1,&model::SemanticMemoryLoadV1,
            &model::SemanticCheckedBinaryRvalueV1,&model::SemanticUncheckedBinaryRvalueV1,
            &model::SemanticAggregateRvalueV1,&model::SemanticAtomicRmwV1,
            &model::SemanticAtomicCompareExchangeV1,&model::SemanticDirectTailCallV1,
            &SemanticRvalueKindV1,&SemanticLocalIdV1,&SemanticOperandV1,&SemanticOperandV1,
            SemanticVolatilityV1,Option<model::SemanticAtomicAccessV1>,
            &[SemanticProjectionV1],&[SemanticOperandV1],&[SemanticLocalDeclV1],
            &[SemanticBasicBlockV1],&[SemanticStatementV1],&model::SemanticTerminatorV1,
            usize,usize,Option<usize>,R<usize>)>(),
        crate::production_ranked_projection_v1::bf16_nominal_preparation_resources_v1::retained_custody_snapshot_frame_v1(),
        size_of::<([usize;15],std::array::IntoIter<usize,15>,usize,usize,
            Option<usize>,R<[usize;15]>,R<usize>,Error)>(),
    ];
    rows.into_iter().try_fold(0usize, |sum, row| {
        sum.checked_add(row).ok_or_else(arithmetic)
    })
}
#[cfg(test)]
#[path = "retained_scalar_singleton_v1_tests.rs"]
mod tests;
