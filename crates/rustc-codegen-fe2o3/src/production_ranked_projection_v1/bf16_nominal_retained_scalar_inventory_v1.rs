//! One-shot retained scalar-inventory DATA; no authentic-source or readiness token.
//! Original return API/predicates/debits remain unchanged. Caller owns credits.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_preparation_resources_v1::resource;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkLedgerIdentityV1,
};
use std::mem::size_of;

type Error = ProductionRankedProjectionErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Resources<'b, 'w> = PreparationResourcesV1<'b, 'w>;
type Ledger = (usize, CanonicalKernelIrWorkLedgerIdentityV1);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Fresh,
    Terminal,
    Complete,
}

/// Inert component only. A genuine factory must construct/lend this inside its
/// original checked owner loan; equal caller-supplied function DATA is not proof.
/// All partial inventory and current block candidates remain physically owned.
pub(in crate::production_ranked_projection_v1) struct RetainedScalarInventoryV1 {
    phase: Phase,
    function: Option<usize>,
    ledger: Option<Ledger>,
    inventory: AssertionDefinitionInventoryV1,
    block_candidate: Vec<usize>,
}
impl RetainedScalarInventoryV1 {
    pub(in crate::production_ranked_projection_v1) fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            function: None,
            ledger: None,
            inventory: AssertionDefinitionInventoryV1 {
                counts: Vec::new(),
                blocks: Vec::new(),
                assignments: Vec::new(),
                address_escaped: Vec::new(),
            },
            block_candidate: Vec::new(),
        }
    }
    pub(in crate::production_ranked_projection_v1) fn prepare_into(
        &mut self,
        function: &SemanticFunctionDeclV1,
        resources: &mut Resources<'_, '_>,
    ) -> Result<()> {
        let fresh = self.phase == Phase::Fresh && self.function.is_none() && self.ledger.is_none();
        self.phase = Phase::Terminal;
        if !fresh || !resources.is_metered() || resources.has_denial() {
            return Err(resource(Resource::Accounting));
        }
        let ledger = resources
            .original_ledger_v1()
            .ok_or_else(|| resource(Resource::Accounting))?;
        // Explicit new wrapper work/storage prefix; after this prefix the
        // original donor's complete debit and predicate order is unchanged.
        resources.work(32)?;
        resources.reserve_storage(retained_scalar_frame_v1()?)?;
        self.function = Some(function as *const SemanticFunctionDeclV1 as usize);
        self.ledger = Some(ledger);
        self.prepare_attached(function, resources)?;
        if resources.has_denial() || resources.original_ledger_v1() != Some(ledger) {
            return Err(resource(Resource::Accounting));
        }
        self.phase = Phase::Complete;
        Ok(())
    }
    pub(in crate::production_ranked_projection_v1) fn completed_for<'a>(
        &'a self,
        function: &SemanticFunctionDeclV1,
        resources: &Resources<'_, '_>,
    ) -> Result<&'a AssertionDefinitionInventoryV1> {
        if self.phase != Phase::Complete
            || self.function != Some(function as *const SemanticFunctionDeclV1 as usize)
            || self.ledger.is_none()
            || self.ledger != resources.original_ledger_v1()
            || resources.has_denial()
        {
            return Err(resource(Resource::Accounting));
        }
        Ok(&self.inventory)
    }

    fn prepare_attached(
        &mut self,
        function: &SemanticFunctionDeclV1,
        resources: &mut Resources<'_, '_>,
    ) -> Result<()> {
        resources.reserve_storage(
            std::mem::size_of::<AssertionDefinitionInventoryV1>()
                + std::mem::size_of::<Vec<usize>>()
                + 4096,
        )?;
        let local_count = function.locals().len();
        fill_attached(&mut self.inventory.counts, local_count, 0_u8, resources)?;
        fill_attached(
            &mut self.inventory.assignments,
            local_count,
            None,
            resources,
        )?;
        fill_attached(
            &mut self.inventory.address_escaped,
            local_count,
            false,
            resources,
        )?;
        resources.reserve(&mut self.inventory.blocks, function.blocks().len())?;
        for (block_index, block) in function.blocks().iter().enumerate() {
            resources.work(4)?;
            let capacity = block
                .statements()
                .len()
                .checked_mul(2)
                .and_then(|value| value.checked_add(1))
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "assertion proof block-definition capacity overflowed",
                ))?;
            resources.reserve(&mut self.block_candidate, capacity)?;
            for (statement_index, statement) in block.statements().iter().enumerate() {
                // At most two definition places; their pushes are within the
                // prepaid 2*statements+1 capacity and fixed statement scan.
                resources.work(16)?;
                if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
                    if assignment.destination().projections().is_empty()
                        && let Some(local) = local_definition_index(assignment.destination())
                    {
                        let Some(slot) = self.inventory.assignments.get_mut(local) else {
                            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                                "an assertion proof assignment is outside the semantic local table",
                            ));
                        };
                        if slot.is_none() {
                            *slot = Some(ScalarAssignmentSiteV1 {
                                block: block_index,
                                statement: statement_index,
                            });
                        }
                    }
                    if let Some(slot) = address_escaped_local_index_v1(assignment.value().kind())
                        .and_then(|local| self.inventory.address_escaped.get_mut(local))
                    {
                        *slot = true;
                    }
                }
                visit_statement_definition_places(statement.kind(), &mut |place| {
                    if let Some(local) = local_definition_index(place) {
                        if let Some(slot) = self.inventory.counts.get_mut(local) {
                            *slot = slot.saturating_add(1);
                        }
                        self.block_candidate.push(local);
                    }
                });
            }
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
                && let Some(local) = call
                    .destination()
                    .and_then(|destination| local_definition_index(destination.place()))
            {
                if let Some(slot) = self.inventory.counts.get_mut(local) {
                    *slot = slot.saturating_add(1);
                }
                self.block_candidate.push(local);
            }
            resources.sort_unique_indices(&mut self.block_candidate)?;
            resources.work(1)?;
            resources.reserve(&mut self.inventory.blocks, 1)?;
            self.inventory
                .blocks
                .push(std::mem::take(&mut self.block_candidate));
        }
        for (local, assignment) in self.inventory.assignments.iter_mut().enumerate() {
            resources.work(1)?;
            if self.inventory.counts.get(local).copied() != Some(1) {
                *assignment = None;
            }
        }
        Ok(())
    }
}

fn fill_attached<T: Clone>(
    values: &mut Vec<T>,
    count: usize,
    value: T,
    resources: &mut Resources<'_, '_>,
) -> Result<()> {
    // Exact original resources.filled order, with the destination already owned.
    resources.work(count)?;
    resources.reserve(values, count)?;
    values.resize(count, value);
    Ok(())
}
const FRAME_ROWS: usize = 17;
fn typed_rows() -> Result<[usize; FRAME_ROWS]> {
    Ok([
        size_of::<RetainedScalarInventoryV1>(),
        size_of::<(
            AssertionDefinitionInventoryV1,
            Vec<usize>,
            Phase,
            Option<usize>,
            Option<Ledger>,
        )>(),
        size_of::<(
            &mut RetainedScalarInventoryV1,
            &SemanticFunctionDeclV1,
            &mut Resources<'static, 'static>,
            bool,
            Ledger,
            Option<Ledger>,
            Result<()>,
        )>(),
        size_of::<(
            &RetainedScalarInventoryV1,
            &SemanticFunctionDeclV1,
            &Resources<'static, 'static>,
            Option<usize>,
            Option<Ledger>,
            Result<&AssertionDefinitionInventoryV1>,
        )>(),
        size_of::<(
            &mut RetainedScalarInventoryV1,
            &SemanticFunctionDeclV1,
            &mut Resources<'static, 'static>,
            usize,
            usize,
            usize,
            usize,
            Option<usize>,
            Result<()>,
        )>(),
        fill_frame::<u8>(),
        fill_frame::<Option<ScalarAssignmentSiteV1>>(),
        fill_frame::<bool>(),
        size_of::<(
            &mut Vec<Vec<usize>>,
            usize,
            &mut Resources<'static, 'static>,
            Vec<usize>,
            Result<()>,
        )>(),
        size_of::<(
            &mut Vec<usize>,
            usize,
            &mut Resources<'static, 'static>,
            usize,
            Option<usize>,
            Result<()>,
        )>(),
        size_of::<(
            &mut Vec<u8>,
            &mut Vec<usize>,
            &SemanticPlaceV1,
            Option<usize>,
            Option<&mut u8>,
        )>(),
        size_of::<(
            &mut Option<ScalarAssignmentSiteV1>,
            ScalarAssignmentSiteV1,
            Option<usize>,
            bool,
        )>(),
        size_of::<(&mut Vec<usize>, Vec<usize>, Vec<usize>)>(),
        size_of::<(Error, Resource, Result<()>, Option<Ledger>, bool)>(),
        size_of::<(
            [usize; FRAME_ROWS],
            Result<[usize; FRAME_ROWS]>,
            std::array::IntoIter<usize, FRAME_ROWS>,
            usize,
            usize,
            Option<usize>,
            Result<usize>,
        )>(),
        size_of::<(
            &[usize],
            std::slice::Iter<'static, usize>,
            &usize,
            usize,
            Option<usize>,
        )>(),
        size_of::<(Result<usize>, usize, Option<usize>, Resource, Error)>(),
    ])
}
fn fill_frame<T>() -> usize {
    size_of::<(
        &mut Vec<T>,
        usize,
        T,
        &mut Resources<'static, 'static>,
        usize,
        Option<usize>,
        Result<()>,
        Result<()>,
    )>()
}
pub(in crate::production_ranked_projection_v1) fn retained_scalar_frame_v1() -> Result<usize> {
    typed_rows()?.into_iter().try_fold(0usize, |sum, row| {
        sum.checked_add(row)
            .ok_or_else(|| resource(Resource::Arithmetic))
    })
}

#[cfg(test)]
#[path = "bf16_nominal_retained_scalar_inventory_v1_tests.rs"]
mod tests;
