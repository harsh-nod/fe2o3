//! One-shot retained constants DATA; genuine source authority remains with the caller.
//! No ordinary route change, new Budget, refund, recursion or return-API fallback.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_preparation_resources_v1::resource;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkLedgerIdentityV1,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticBasicBlockV1, SemanticStatementV1};
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

/// All definitions, visited states, resolved values and reusable path remain
/// attached on partial failure. Only an outer checked owner may establish source
/// authority; these lexical tags do not create an authentic source capability.
pub(in crate::production_ranked_projection_v1) struct RetainedConstantLocalsV1 {
    phase: Phase,
    function: Option<usize>,
    ledger: Option<Ledger>,
    definitions: Vec<ConstantDefinitionV1>,
    states: Vec<u8>,
    values: Vec<Option<u64>>,
    path: Vec<usize>,
}
impl RetainedConstantLocalsV1 {
    pub(in crate::production_ranked_projection_v1) fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            function: None,
            ledger: None,
            definitions: Vec::new(),
            states: Vec::new(),
            values: Vec::new(),
            path: Vec::new(),
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
        // Additive wrapper prefix only; original algorithm/debit order follows.
        resources.work(32)?;
        resources.reserve_storage(retained_constant_frame_v1()?)?;
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
    ) -> Result<&'a [Option<u64>]> {
        if self.phase != Phase::Complete
            || self.function != Some(function as *const SemanticFunctionDeclV1 as usize)
            || self.ledger.is_none()
            || self.ledger != resources.original_ledger_v1()
            || resources.has_denial()
        {
            return Err(resource(Resource::Accounting));
        }
        Ok(&self.values)
    }
    fn prepare_attached(
        &mut self,
        function: &SemanticFunctionDeclV1,
        resources: &mut Resources<'_, '_>,
    ) -> Result<()> {
        resources.reserve_storage(
            std::mem::size_of::<Vec<ConstantDefinitionV1>>()
                + std::mem::size_of::<Vec<u8>>()
                + std::mem::size_of::<Vec<Option<u64>>>()
                + std::mem::size_of::<Vec<usize>>()
                + 4096,
        )?;
        let local_count = function.locals().len();
        fill_attached(
            &mut self.definitions,
            local_count,
            ConstantDefinitionV1::Missing,
            resources,
        )?;
        for block in function.blocks() {
            resources.work(4)?;
            for statement in block.statements() {
                resources.work(16)?;
                if let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                    && let Some(definition) =
                        address_escaped_local_index_v1(assignment.value().kind())
                            .and_then(|local| self.definitions.get_mut(local))
                {
                    *definition = ConstantDefinitionV1::Invalid;
                }
                if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
                    if local_definition_index(assignment.destination()).is_some() {
                        record_constant_definition(
                            &mut self.definitions,
                            assignment.destination().local(),
                            match (
                                assignment.destination().projections(),
                                assignment.value().kind(),
                            ) {
                                ([], SemanticRvalueKindV1::Use(operand)) => {
                                    constant_definition(operand)
                                }
                                _ => ConstantDefinitionV1::Invalid,
                            },
                        );
                    }
                } else {
                    visit_statement_definition_places(statement.kind(), &mut |place| {
                        if local_definition_index(place).is_some() {
                            record_constant_definition(
                                &mut self.definitions,
                                place.local(),
                                ConstantDefinitionV1::Invalid,
                            );
                        }
                    });
                }
            }
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
                && let Some(destination) = call.destination()
                && local_definition_index(destination.place()).is_some()
            {
                record_constant_definition(
                    &mut self.definitions,
                    destination.place().local(),
                    ConstantDefinitionV1::Invalid,
                );
            }
        }
        fill_attached(&mut self.states, local_count, 0_u8, resources)?;
        fill_attached(&mut self.values, local_count, None, resources)?;
        resources.reserve(&mut self.path, local_count)?;
        for index in 0..self.definitions.len() {
            resolve_constant_iterative_with_resources_v1(
                index,
                &self.definitions,
                &mut self.states,
                &mut self.values,
                &mut self.path,
                resources,
            )?;
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
    // Exact filled() debit/reserve/resize sequence, with destination already owned.
    resources.work(count)?;
    resources.reserve(values, count)?;
    values.resize(count, value);
    Ok(())
}
const FRAME_ROWS: usize = 21;
fn typed_rows() -> Result<[usize; FRAME_ROWS]> {
    Ok([
        // owner
        size_of::<RetainedConstantLocalsV1>(),
        // constructor
        size_of::<(
            Phase,
            Option<usize>,
            Option<Ledger>,
            Vec<ConstantDefinitionV1>,
            Vec<u8>,
            Vec<Option<u64>>,
            Vec<usize>,
        )>(),
        // entry
        size_of::<(
            &mut RetainedConstantLocalsV1,
            &SemanticFunctionDeclV1,
            &mut Resources<'static, 'static>,
            bool,
            Ledger,
            Option<Ledger>,
            Result<()>,
        )>(),
        // completed-loan
        size_of::<(
            &RetainedConstantLocalsV1,
            &SemanticFunctionDeclV1,
            &Resources<'static, 'static>,
            Option<usize>,
            Option<Ledger>,
            &Vec<Option<u64>>,
            &[Option<u64>],
            Result<&[Option<u64>]>,
        )>(),
        // scan-entry
        size_of::<(
            &mut RetainedConstantLocalsV1,
            &SemanticFunctionDeclV1,
            &mut Resources<'static, 'static>,
            usize,
            Result<()>,
        )>(),
        // fill-definitions
        fill_frame::<ConstantDefinitionV1>(),
        // fill-states
        fill_frame::<u8>(),
        // fill-values
        fill_frame::<Option<u64>>(),
        // source-iterators
        size_of::<(
            std::slice::Iter<'static, SemanticBasicBlockV1>,
            &SemanticBasicBlockV1,
            std::slice::Iter<'static, SemanticStatementV1>,
            &SemanticStatementV1,
            &SemanticStatementKindV1,
        )>(),
        // assignment-borrows
        size_of::<(
            &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
            &SemanticPlaceV1,
            &SemanticRvalueV1,
            &SemanticRvalueKindV1,
            &[fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1],
            &SemanticOperandV1,
            ConstantDefinitionV1,
        )>(),
        // escaped-definition-closure
        size_of::<(
            &mut Vec<ConstantDefinitionV1>,
            &mut [ConstantDefinitionV1],
            usize,
            Option<usize>,
            &mut ConstantDefinitionV1,
            Option<&mut ConstantDefinitionV1>,
        )>(),
        // record-definition-call
        size_of::<(
            &mut Vec<ConstantDefinitionV1>,
            &mut [ConstantDefinitionV1],
            SemanticLocalIdV1,
            ConstantDefinitionV1,
        )>(),
        // other-definition-closure
        size_of::<(
            &mut Vec<ConstantDefinitionV1>,
            &mut [ConstantDefinitionV1],
            &SemanticPlaceV1,
            Option<usize>,
            SemanticLocalIdV1,
            ConstantDefinitionV1,
        )>(),
        // call-destination
        size_of::<(
            &SemanticTerminatorKindV1,
            &SemanticDirectCallV1,
            Option<&fe2o3_mir_model::semantic_mir_v1::SemanticCallDestinationV1>,
            &fe2o3_mir_model::semantic_mir_v1::SemanticCallDestinationV1,
            &SemanticPlaceV1,
            Option<usize>,
        )>(),
        // path-reservation
        size_of::<(
            &mut Vec<usize>,
            usize,
            &mut Resources<'static, 'static>,
            usize,
            Option<usize>,
            Result<()>,
        )>(),
        // resolve-loop
        size_of::<(
            std::ops::Range<usize>,
            usize,
            Result<Option<u64>>,
            Option<u64>,
        )>(),
        // resolve-caller-and-coerced-aliases
        size_of::<(
            usize,
            &Vec<ConstantDefinitionV1>,
            &[ConstantDefinitionV1],
            &mut Vec<u8>,
            &mut [u8],
            &mut Vec<Option<u64>>,
            &mut [Option<u64>],
            &mut Vec<usize>,
            &mut Resources<'static, 'static>,
        )>(),
        // ledger-source-tags
        size_of::<(
            usize,
            Option<usize>,
            Ledger,
            Option<Ledger>,
            bool,
            bool,
            bool,
        )>(),
        // errors
        size_of::<(Error, Resource, Result<()>, Option<Ledger>)>(),
        // accounting-array-and-fold
        size_of::<(
            [usize; FRAME_ROWS],
            Result<[usize; FRAME_ROWS]>,
            std::array::IntoIter<usize, FRAME_ROWS>,
            usize,
            usize,
            Option<usize>,
            Result<usize>,
        )>(),
        // checked-frame-result
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
pub(in crate::production_ranked_projection_v1) fn retained_constant_frame_v1() -> Result<usize> {
    typed_rows()?.into_iter().try_fold(0usize, |sum, row| {
        sum.checked_add(row)
            .ok_or_else(|| resource(Resource::Arithmetic))
    })
}
#[cfg(test)]
#[path = "bf16_nominal_retained_constant_locals_v1_tests.rs"]
mod tests;
