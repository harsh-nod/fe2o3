//! Source-ordered direct LessThan component. Borrowed results are DATA only.
//! The outer owner retains this storage, real prefix mutations and original
//! accepted-credit counter through all postflights/errors/unwinds, then drops
//! physical owners before refund. No all-producer/source-factory authority.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as WorkIdentity,
};
use std::mem::size_of;
type Error = ProductionRankedProjectionErrorV1;
type LedgerIdentity = (usize, WorkIdentity);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Fresh,
    Failed,
    Ready,
}

pub(super) struct DirectComparisonPreparationV1 {
    phase: Phase,
    function: usize,
    ledger: Option<LedgerIdentity>,
    predicates: Vec<Option<GuardPredicateV1>>,
    pending: Option<GuardPredicateV1>,
    duplicates: Vec<GuardPredicateV1>,
}
impl DirectComparisonPreparationV1 {
    pub(super) fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            function: 0,
            ledger: None,
            predicates: Vec::new(),
            pending: None,
            duplicates: Vec::new(),
        }
    }

    /// All slices and mutable producer state are DATA from the caller. The
    /// function/ledger join below is lexical, not authentication of that state
    /// or of the adapter's owned counter. No empty predecessor state is made.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare(
        &mut self,
        function: &SemanticFunctionDeclV1,
        constants: &[Option<u64>],
        stable_argument_origins: &[Option<u32>],
        local_definitions: &[u8],
        address_escaped: &[bool],
        runtime_index_arguments: &mut [Option<u32>],
        next_runtime_argument: &mut usize,
        operations: &mut Vec<ProductionRankedOperationV1>,
        next_value: &mut u32,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<(), Error> {
        if self.phase != Phase::Fresh {
            self.phase = Phase::Failed;
            return Err(unavailable());
        }
        // Set terminal failure before every admission, allocation and source
        // operation. An unwind cannot leave a partially initialized Ready owner.
        self.phase = Phase::Failed;
        if !resources.is_metered() || resources.has_denial() {
            return Err(unavailable());
        }
        self.ledger = resources.original_ledger_v1();
        self.function = function as *const SemanticFunctionDeclV1 as usize;
        let header = direct_frame_v1()?;
        resources.work(header)?;
        resources.reserve_storage(header)?;
        self.prepare_inner(
            function,
            constants,
            stable_argument_origins,
            local_definitions,
            address_escaped,
            runtime_index_arguments,
            next_runtime_argument,
            operations,
            next_value,
            resources,
        )?;
        self.phase = Phase::Ready;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_inner(
        &mut self,
        function: &SemanticFunctionDeclV1,
        constants: &[Option<u64>],
        stable_argument_origins: &[Option<u32>],
        local_definitions: &[u8],
        address_escaped: &[bool],
        runtime_index_arguments: &mut [Option<u32>],
        next_runtime_argument: &mut usize,
        operations: &mut Vec<ProductionRankedOperationV1>,
        next_value: &mut u32,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<(), Error> {
        let local_count = function.locals().len();
        // Reserve directly into the retained owner, never a temporary filled Vec.
        resources.work(local_count)?;
        resources.reserve(&mut self.predicates, local_count)?;
        self.predicates.resize_with(local_count, || None);
        for block in function.blocks() {
            resources.work(1)?;
            for statement in block.statements() {
                resources.work(1)?;
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                if !assignment.destination().projections().is_empty() {
                    continue;
                }
                let SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left,
                    right,
                } = assignment.value().kind()
                else {
                    continue;
                };
                let destination = assignment.destination().local().index() as usize;
                resources.work(1)?;
                if local_definitions.get(destination).copied() != Some(1) {
                    continue;
                }
                resources.work(1)?;
                if address_escaped.get(destination).copied() != Some(false) {
                    continue;
                }
                let Some(lhs) = root_uniform_operand_preparation_v1::uniform_operand_paid_v1(
                    left,
                    constants,
                    stable_argument_origins,
                    runtime_index_arguments,
                    next_runtime_argument,
                    operations,
                    next_value,
                    resources,
                )?
                else {
                    continue;
                };
                let Some(rhs) = root_uniform_operand_preparation_v1::uniform_operand_paid_v1(
                    right,
                    constants,
                    stable_argument_origins,
                    runtime_index_arguments,
                    next_runtime_argument,
                    operations,
                    next_value,
                    resources,
                )?
                else {
                    continue;
                };
                self.retain_candidate(destination, lhs, rhs, resources)?;
            }
        }
        Ok(())
    }

    fn retain_candidate(
        &mut self,
        destination: usize,
        lhs: ProductionRankedValueV1,
        rhs: ProductionRankedValueV1,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<(), Error> {
        // The original call evaluates its fallible destination argument before
        // constructing the candidate. Preserve that exact semantic error order.
        resources.work(1)?;
        let slot = self
            .predicates
            .get_mut(destination)
            .ok_or(Error::Unsupported(
                "a direct switch predicate outside the semantic local table",
            ))?;
        resources.work(1)?;
        self.pending = Some(GuardPredicateV1 {
            comparisons: Vec::new(),
        });
        let candidate = self.pending.as_mut().ok_or_else(unavailable)?;
        resources.work(1)?;
        resources.reserve(&mut candidate.comparisons, 1)?;
        candidate.comparisons.push((lhs, rhs));
        resources.work(1)?;
        match slot {
            None => *slot = self.pending.take(),
            Some(existing) => {
                // Guard equality checks Vec length then bounded scalar pairs.
                // Pay the reached comparison before inspecting pair values.
                let comparisons = existing.comparisons.len().min(candidate.comparisons.len());
                let work = comparisons
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(arithmetic)?;
                resources.work(work)?;
                if *existing != *candidate {
                    // Keep the conflicting candidate in pending on refusal.
                    return Err(Error::Incomplete(
                        "one comparison local has conflicting source definitions",
                    ));
                }
                // Reserve while pending still owns its comparison allocation.
                // Taking it before a fallible push would drop it on refusal.
                resources.work(1)?;
                resources.reserve(&mut self.duplicates, 1)?;
                let retained = self.pending.take().ok_or_else(unavailable)?;
                self.duplicates.push(retained);
            }
        }
        Ok(())
    }

    /// Borrowed DATA, not authority to skip later producer phases.
    pub(super) fn view<'a>(
        &'a mut self,
        function: &SemanticFunctionDeclV1,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<&'a [Option<GuardPredicateV1>], Error> {
        if self.phase != Phase::Ready
            || resources.has_denial()
            || self.ledger.is_none()
            || self.ledger != resources.original_ledger_v1()
            || self.function != function as *const SemanticFunctionDeclV1 as usize
        {
            self.phase = Phase::Failed;
            return Err(unavailable());
        }
        if let Err(error) = resources.work(1) {
            self.phase = Phase::Failed;
            return Err(error);
        }
        Ok(&self.predicates)
    }
}
fn unavailable() -> Error {
    Error::Unsupported("direct comparison component is unavailable for this source and ledger")
}
fn arithmetic() -> Error {
    bf16_nominal_preparation_resources_v1::resource(Resource::Arithmetic)
}
fn sum(parts: &[usize]) -> Result<usize, Error> {
    parts
        .iter()
        .try_fold(0usize, |n, x| n.checked_add(*x).ok_or_else(arithmetic))
}
fn call_frame<T>(locals: usize) -> Result<usize, Error> {
    sum(&[
        locals,
        size_of::<T>(),
        size_of::<T>(),
        size_of::<Result<T, Error>>(),
        size_of::<Result<T, Error>>(),
    ])
}

const SOURCE_ACCESSOR_ROWS: usize = 11;
fn source_accessor_rows_v1() -> Result<[usize; SOURCE_ACCESSOR_ROWS], Error> {
    use fe2o3_mir_model::semantic_mir_v1 as mir;
    Ok([
        // A0: overlapping source for-loop iterator/view/next-result slots.
        // Iterator implementation frames are not selected model source.
        call_frame::<()>(size_of::<(
            &[mir::SemanticBasicBlockV1],
            std::slice::Iter<'_, mir::SemanticBasicBlockV1>,
            Option<&mir::SemanticBasicBlockV1>,
            &mir::SemanticBasicBlockV1,
            &[mir::SemanticStatementV1],
            std::slice::Iter<'_, mir::SemanticStatementV1>,
            Option<&mir::SemanticStatementV1>,
            &mir::SemanticStatementV1,
            bool,
        )>())?,
        // A1: function.locals() receiver and borrowed slice return.
        call_frame::<&[mir::SemanticLocalDeclV1]>(size_of::<&SemanticFunctionDeclV1>())?,
        // A2: function.blocks() is a separate receiver/return vertex.
        call_frame::<&[mir::SemanticBasicBlockV1]>(size_of::<&SemanticFunctionDeclV1>())?,
        // A3: block.statements() while the outer block reference is live.
        call_frame::<&[mir::SemanticStatementV1]>(size_of::<&mir::SemanticBasicBlockV1>())?,
        // A4: statement.kind(), not a unit-returning frame.
        call_frame::<&mir::SemanticStatementKindV1>(size_of::<&mir::SemanticStatementV1>())?,
        // A5: assignment.destination() with its distinct assignment receiver.
        call_frame::<&SemanticPlaceV1>(size_of::<&mir::SemanticAssignmentV1>())?,
        // A6: assignment.value(), independently live from its outer receiver.
        call_frame::<&mir::SemanticRvalueV1>(size_of::<&mir::SemanticAssignmentV1>())?,
        // A7: rvalue.kind() while its borrowed rvalue result is live.
        call_frame::<&SemanticRvalueKindV1>(size_of::<&mir::SemanticRvalueV1>())?,
        // A8: place.projections() with borrowed slice return.
        call_frame::<&[mir::SemanticProjectionV1]>(size_of::<&SemanticPlaceV1>())?,
        // A9: place.local() returns the scalar ID by value.
        call_frame::<SemanticLocalIdV1>(size_of::<&SemanticPlaceV1>())?,
        // A10: index_id!'s index(self) consumes a distinct Copy ID receiver.
        call_frame::<u32>(size_of::<SemanticLocalIdV1>())?,
    ])
}
const FRAME_ROWS: usize = 17;
fn frame_rows() -> Result<[usize; FRAME_ROWS], Error> {
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    use fe2o3_mir_model::semantic_mir_v1 as mir;
    type Pair = (ProductionRankedValueV1, ProductionRankedValueV1);
    Ok([
        // 0: caller-retained component's real fixed owner, no heap payload.
        size_of::<DirectComparisonPreparationV1>(),
        // 1: public one-shot entry and all retained original input slots.
        call_frame::<()>(size_of::<(
            &mut DirectComparisonPreparationV1,
            &SemanticFunctionDeclV1,
            &[Option<u64>],
            &[Option<u32>],
            &[u8],
            &[bool],
            &mut [Option<u32>],
            &mut usize,
            &mut Vec<ProductionRankedOperationV1>,
            &mut u32,
            &mut PreparationResourcesV1<'_, '_>,
            Option<LedgerIdentity>,
            usize,
            bool,
            Result<(), Error>,
        )>())?,
        // 2: inner source loop arguments and branch-local value transfers.
        call_frame::<()>(size_of::<(
            &mut DirectComparisonPreparationV1,
            &SemanticFunctionDeclV1,
            &[Option<u64>],
            &[Option<u32>],
            &[u8],
            &[bool],
            &mut [Option<u32>],
            &mut usize,
            &mut Vec<ProductionRankedOperationV1>,
            &mut u32,
            &mut PreparationResourcesV1<'_, '_>,
            usize,
            usize,
            Option<&u8>,
            Option<u8>,
            Option<&bool>,
            Option<bool>,
            &mir::SemanticAssignmentV1,
            &SemanticOperandV1,
            &SemanticOperandV1,
            Option<ProductionRankedValueV1>,
            ProductionRankedValueV1,
            Option<ProductionRankedValueV1>,
            ProductionRankedValueV1,
        )>())?,
        // 3: explicit selected model callees, plus their roster constructor
        // and returned array/result/borrowed sum input (no residual padding).
        sum(&[
            sum(&source_accessor_rows_v1()?)?,
            call_frame::<[usize; SOURCE_ACCESSOR_ROWS]>(size_of::<(
                [usize; SOURCE_ACCESSOR_ROWS],
                &[usize],
                usize,
                [usize; 2],
                Result<usize, Error>,
            )>())?,
        ])?,
        // 4: each reached operand bridge. Its callee pays its own corrected H
        // per invocation; this row is only the overlapping caller transfer.
        call_frame::<Option<ProductionRankedValueV1>>(size_of::<(
            &SemanticOperandV1,
            &[Option<u64>],
            &[Option<u32>],
            &mut [Option<u32>],
            &mut usize,
            &mut Vec<ProductionRankedOperationV1>,
            &mut u32,
            &mut PreparationResourcesV1<'_, '_>,
            Option<ProductionRankedValueV1>,
        )>())?,
        // 5: destination lookup, attached candidate, checked comparison charge,
        // reserved duplicate append, including pending's moved header.
        call_frame::<()>(size_of::<(
            &mut DirectComparisonPreparationV1,
            usize,
            ProductionRankedValueV1,
            ProductionRankedValueV1,
            &mut PreparationResourcesV1<'_, '_>,
            &mut Option<GuardPredicateV1>,
            &mut GuardPredicateV1,
            &mut GuardPredicateV1,
            GuardPredicateV1,
            Option<GuardPredicateV1>,
            Pair,
            usize,
            usize,
            Option<usize>,
            bool,
            Option<&mut Option<GuardPredicateV1>>,
            Result<&mut Option<GuardPredicateV1>, Error>,
            Option<&mut GuardPredicateV1>,
            Result<&mut GuardPredicateV1, Error>,
            Result<GuardPredicateV1, Error>,
            Error,
        )>())?,
        // 6: selected logical Guard/pair/scalar equality interface only.
        // Std Vec/slice Eq and iterator implementation frames and generated
        // native equality stack are explicitly outside this source model.
        // Ranked values are the pinned three scalar-only variants.
        call_frame::<bool>(size_of::<(
            &GuardPredicateV1,
            &GuardPredicateV1,
            &[Pair],
            &[Pair],
            &Pair,
            &Pair,
            &ProductionRankedValueV1,
            &ProductionRankedValueV1,
            ProductionRankedValueV1,
            ProductionRankedValueV1,
            usize,
            bool,
        )>())?,
        // 7: borrowed result entry; the retained H remains live through views.
        call_frame::<&[Option<GuardPredicateV1>]>(size_of::<(
            &mut DirectComparisonPreparationV1,
            &SemanticFunctionDeclV1,
            &mut PreparationResourcesV1<'_, '_>,
            Option<LedgerIdentity>,
            usize,
            bool,
            Error,
        )>())?,
        // 8: existing preparation work adapter, not a new budget policy.
        call_frame::<()>(size_of::<(
            &mut PreparationResourcesV1<'_, '_>,
            usize,
            &mut Budget<'_>,
            Result<(), Resource>,
        )>())?,
        // 9: existing storage adapter and cumulative owned-counter check.
        call_frame::<()>(size_of::<(
            &mut PreparationResourcesV1<'_, '_>,
            usize,
            &mut Budget<'_>,
            &mut usize,
            usize,
            Option<usize>,
            Result<(), Resource>,
        )>())?,
        // 10: original exact reserve instantiated for all three owner vectors.
        sum(&[
            reserve_frame::<Option<GuardPredicateV1>>()?,
            reserve_frame::<Pair>()?,
            reserve_frame::<GuardPredicateV1>()?,
        ])?,
        // 11: lexical original-ledger / mode / sticky-denial adapter queries.
        sum(&[
            call_frame::<Option<LedgerIdentity>>(size_of::<(
                &PreparationResourcesV1<'_, '_>,
                &Budget<'_>,
                usize,
                WorkIdentity,
            )>())?,
            call_frame::<bool>(size_of::<(&PreparationResourcesV1<'_, '_>, bool)>())?,
            call_frame::<bool>(size_of::<(
                &PreparationResourcesV1<'_, '_>,
                &Budget<'_>,
                Option<usize>,
                bool,
            )>())?,
        ])?,
        // 12: roster/duplicate/pending owner mutation inputs; vector payload
        // and relocation are admitted separately by the original reserve.
        call_frame::<()>(size_of::<(
            &mut Vec<Option<GuardPredicateV1>>,
            usize,
            Option<GuardPredicateV1>,
            &mut Vec<GuardPredicateV1>,
            &mut Option<GuardPredicateV1>,
            GuardPredicateV1,
            &mut Vec<Pair>,
            Pair,
        )>())?,
        // 13: complete roster constructor and public frame-query transfers.
        call_frame::<[usize; FRAME_ROWS]>(size_of::<(
            [usize; FRAME_ROWS],
            &[usize],
            usize,
            Result<usize, Error>,
        )>())?,
        // 14: checked sum and call_frame each have a distinct fixed vertex.
        sum(&[
            call_frame::<usize>(size_of::<(
                &[usize],
                std::slice::Iter<'_, usize>,
                usize,
                &usize,
                Option<usize>,
            )>())?,
            call_frame::<usize>(size_of::<(usize, [usize; 5], &[usize])>())?,
        ])?,
        // 15: typed reserve-frame constructor and its call_frame result.
        call_frame::<usize>(size_of::<(usize, Result<usize, Error>)>())?,
        // 16: semantic refusal, arithmetic and resource mapper are distinct.
        sum(&[
            call_frame::<Error>(size_of::<(&'static str, Error)>())?,
            call_frame::<Error>(size_of::<Resource>())?,
            call_frame::<Error>(size_of::<(Resource, CanonicalAssertionErrorV1)>())?,
        ])?,
    ])
}
fn reserve_frame<T>() -> Result<usize, Error> {
    call_frame::<()>(size_of::<(
        &mut PreparationResourcesV1<'_, '_>,
        &mut Vec<T>,
        usize,
        usize,
        Option<usize>,
        bool,
        usize,
        std::collections::TryReserveError,
        Result<(), std::collections::TryReserveError>,
    )>())
}
pub(super) fn direct_frame_v1() -> Result<usize, Error> {
    sum(&frame_rows()?)
}

#[cfg(test)]
pub(super) mod test_access {
    use super::*;
    #[allow(clippy::type_complexity)]
    pub(in crate::production_ranked_projection_v1) fn observe(
        owner: &DirectComparisonPreparationV1,
    ) -> (
        u8,
        Vec<Option<GuardPredicateV1>>,
        Option<GuardPredicateV1>,
        Vec<GuardPredicateV1>,
    ) {
        let phase = match owner.phase {
            Phase::Fresh => 0,
            Phase::Failed => 1,
            Phase::Ready => 2,
        };
        (
            phase,
            owner.predicates.clone(),
            owner.pending.clone(),
            owner.duplicates.clone(),
        )
    }
    pub(in crate::production_ranked_projection_v1) fn payload_pointers(
        owner: &DirectComparisonPreparationV1,
    ) -> Vec<usize> {
        owner
            .predicates
            .iter()
            .flatten()
            .chain(owner.pending.iter())
            .chain(owner.duplicates.iter())
            .map(|g| g.comparisons.as_ptr() as usize)
            .collect()
    }
    pub(in crate::production_ranked_projection_v1) fn capacities(
        owner: &DirectComparisonPreparationV1,
    ) -> (usize, usize, usize) {
        (
            owner.predicates.capacity(),
            owner
                .pending
                .as_ref()
                .map_or(0, |x| x.comparisons.capacity()),
            owner.duplicates.capacity(),
        )
    }
    pub(in crate::production_ranked_projection_v1) fn source_accessor_frames() {
        use fe2o3_mir_model::semantic_mir_v1 as mir;
        // Independent typed expected values, not calls to call_frame.
        fn expected<T>(locals: usize) -> usize {
            locals + 2 * size_of::<T>() + 2 * size_of::<Result<T, Error>>()
        }
        let expected_rows = [
            expected::<()>(size_of::<(
                &[mir::SemanticBasicBlockV1],
                std::slice::Iter<'_, mir::SemanticBasicBlockV1>,
                Option<&mir::SemanticBasicBlockV1>,
                &mir::SemanticBasicBlockV1,
                &[mir::SemanticStatementV1],
                std::slice::Iter<'_, mir::SemanticStatementV1>,
                Option<&mir::SemanticStatementV1>,
                &mir::SemanticStatementV1,
                bool,
            )>()),
            expected::<&[mir::SemanticLocalDeclV1]>(size_of::<&SemanticFunctionDeclV1>()),
            expected::<&[mir::SemanticBasicBlockV1]>(size_of::<&SemanticFunctionDeclV1>()),
            expected::<&[mir::SemanticStatementV1]>(size_of::<&mir::SemanticBasicBlockV1>()),
            expected::<&mir::SemanticStatementKindV1>(size_of::<&mir::SemanticStatementV1>()),
            expected::<&SemanticPlaceV1>(size_of::<&mir::SemanticAssignmentV1>()),
            expected::<&mir::SemanticRvalueV1>(size_of::<&mir::SemanticAssignmentV1>()),
            expected::<&SemanticRvalueKindV1>(size_of::<&mir::SemanticRvalueV1>()),
            expected::<&[mir::SemanticProjectionV1]>(size_of::<&SemanticPlaceV1>()),
            expected::<SemanticLocalIdV1>(size_of::<&SemanticPlaceV1>()),
            expected::<u32>(size_of::<SemanticLocalIdV1>()),
        ];
        assert_eq!(source_accessor_rows_v1().unwrap(), expected_rows);
        let construction = expected::<[usize; SOURCE_ACCESSOR_ROWS]>(size_of::<(
            [usize; SOURCE_ACCESSOR_ROWS],
            &[usize],
            usize,
            [usize; 2],
            Result<usize, Error>,
        )>());
        assert_eq!(
            frame_rows().unwrap()[3],
            expected_rows.iter().sum::<usize>() + construction
        );
        for row in expected_rows {
            assert!(row > 0);
            assert!(sum(&[usize::MAX, row]).is_err());
        }
        assert!(sum(&[usize::MAX, construction]).is_err());
    }
    pub(in crate::production_ranked_projection_v1) fn frames() {
        let rows = frame_rows().unwrap();
        assert_eq!(rows.len(), 17);
        assert!(rows.iter().all(|n| *n > 0));
        assert_eq!(sum(&rows).unwrap(), direct_frame_v1().unwrap());
        for row in rows {
            assert!(sum(&[usize::MAX, row]).is_err());
        }
        assert!(call_frame::<()>(usize::MAX).is_err());
        type Pair = (ProductionRankedValueV1, ProductionRankedValueV1);
        let pair = size_of::<(
            &mut PreparationResourcesV1<'_, '_>,
            &mut Vec<Pair>,
            usize,
            usize,
            Option<usize>,
            bool,
            usize,
            std::collections::TryReserveError,
            Result<(), std::collections::TryReserveError>,
        )>() + 2 * size_of::<()>()
            + 2 * size_of::<Result<(), Error>>();
        assert_eq!(reserve_frame::<Pair>().unwrap(), pair);
        assert_eq!(rows[0], size_of::<DirectComparisonPreparationV1>());
    }
}
