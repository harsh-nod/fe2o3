//! Inert retained scalar-borrow census; no source, query, or routing authority.
//! Original candidate/Scan/resolve remain unchanged in the parent module.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_preparation_resources_v1::{
    PreparationCustodySnapshotV1 as Snapshot, PreparationResourcesV1 as Prep,
};
use fe2o3_mir_model::semantic_mir_v1 as model;
use std::mem::size_of;
type Error = ProductionRankedProjectionErrorV1;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Fresh,
    Terminal,
    Complete,
}
#[derive(Clone, Copy)]
struct Source<'a> {
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    target: SemanticTargetDataLayoutV1,
}
fn accounting() -> Error {
    resource(Resource::Accounting)
}
fn arithmetic() -> Error {
    resource(Resource::Arithmetic)
}
fn unavailable() -> Error {
    Error::Incomplete("retained scalar-borrow adapter cannot answer semantic queries")
}
pub(in crate::production_ranked_projection_v1) struct RetainedScalarBorrowsV1<'a> {
    phase: Phase,
    source: Option<Source<'a>>,
    entry: Option<Snapshot>,
    held: Option<Snapshot>,
    census: Option<ScalarPrivateBorrowsV1<'a>>,
    scan_invoked: bool,
    found: bool,
    statements: usize,
}
impl<'a> RetainedScalarBorrowsV1<'a> {
    pub(in crate::production_ranked_projection_v1) fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            source: None,
            entry: None,
            held: None,
            census: None,
            scan_invoked: false,
            found: false,
            statements: 0,
        }
    }
    pub(in crate::production_ranked_projection_v1) fn prepare_into(
        &mut self,
        types: &'a [SemanticTypeDeclV1],
        function: &'a SemanticFunctionDeclV1,
        target: SemanticTargetDataLayoutV1,
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
        self.source = Some(Source {
            types,
            function,
            target,
        });
        self.entry = Some(entry);
        resources.work(32)?;
        resources.reserve_storage(frame()?)?;
        self.prepare_original(types, function, target, resources)?;
        self.check(types, function, target, resources)?;
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
        types: &'a [SemanticTypeDeclV1],
        function: &'a SemanticFunctionDeclV1,
        target: SemanticTargetDataLayoutV1,
        resources: &mut Prep<'_, '_>,
    ) -> R<()> {
        let mut meter = Meter(resources);
        let facts: &mut dyn ProjectedAssertionFactsV1 = &mut meter;
        charge(facts, 6)?;
        self.scan_invoked = true;
        for (block, body) in function.blocks().iter().enumerate() {
            charge(facts, 3)?;
            self.statements = self
                .statements
                .checked_add(body.statements().len())
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            for (statement, value) in body.statements().iter().enumerate() {
                self.found |= candidate(
                    types,
                    function,
                    target,
                    block,
                    statement,
                    value.kind(),
                    facts,
                )?
                .is_some();
            }
        }
        if !self.found {
            return Ok(());
        }
        let entry = self.entry.ok_or_else(accounting)?;
        self.census = Some(ScalarPrivateBorrowsV1 {
            function,
            types,
            target,
            ledger: (entry.budget_slot, entry.work_ledger),
            live_floor: 0,
            locals: Vec::new(),
            starts: Vec::new(),
            reads: Vec::new(),
        });
        build_into(
            self.census.as_mut().ok_or_else(accounting)?,
            types,
            function,
            target,
            self.statements,
            facts,
        )
    }

    fn check(
        &self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        target: SemanticTargetDataLayoutV1,
        resources: &Prep<'_, '_>,
    ) -> R<()> {
        let source = self.source.ok_or_else(accounting)?;
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
        if !std::ptr::eq(source.types, types)
            || !std::ptr::eq(source.function, function)
            || source.target != target
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
    pub(in crate::production_ranked_projection_v1) fn completed_for<'s>(
        &'s self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        target: SemanticTargetDataLayoutV1,
        resources: &Prep<'_, '_>,
    ) -> R<Option<&'s ScalarPrivateBorrowsV1<'a>>> {
        if self.phase != Phase::Complete {
            return Err(accounting());
        }
        self.check(types, function, target, resources)?;
        match self.census.as_ref() {
            None if !self.found => Ok(None),
            Some(census) if self.found => {
                let expected_starts = function
                    .blocks()
                    .len()
                    .checked_add(1)
                    .ok_or_else(arithmetic)?;
                if !std::ptr::eq(census.function, function)
                    || !std::ptr::eq(census.types, types)
                    || census.target != target
                    || census.locals.len() != function.locals().len()
                    || census.starts.len() != expected_starts
                    || census.reads.len() != self.statements
                {
                    return Err(accounting());
                }
                Ok(Some(census))
            }
            _ => Err(accounting()),
        }
    }
}
/// Only the original work/storage/identity methods are forwarded. No Budget
/// loan, refund, replacement ledger or semantic facts are supplied.
struct Meter<'r, 'b, 'w>(&'r mut Prep<'b, 'w>);
impl ProjectedAssertionFactsV1 for Meter<'_, '_, '_> {
    fn charge_private_array_work(&mut self, amount: usize) -> R<()> {
        self.0.work(amount)
    }
    fn scalar_private_storage_v1(&self) -> R<usize> {
        Ok(self
            .0
            .retained_custody_snapshot_v1()
            .ok_or_else(accounting)?
            .storage)
    }
    fn reserve_scalar_private_storage_v1(&mut self, amount: usize) -> R<()> {
        self.0.reserve_storage(amount)
    }
    fn release_scalar_private_storage_v1(&mut self, _amount: usize) -> R<()> {
        Err(unavailable())
    }
    fn helper_value_ledger_v1(&self) -> R<(usize, CanonicalKernelIrWorkLedgerIdentityV1)> {
        let value = self
            .0
            .retained_custody_snapshot_v1()
            .ok_or_else(accounting)?;
        Ok((value.budget_slot, value.work_ledger))
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

fn allocate_into<T: Clone>(
    output: &mut Vec<T>,
    count: usize,
    value: T,
    facts: &mut dyn ProjectedAssertionFactsV1,
) -> R<()> {
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    facts.reserve_scalar_private_storage_v1(bytes)?;
    output
        .try_reserve_exact(count)
        .map_err(|_| resource(Resource::Allocation))?;
    let actual = output
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    facts.reserve_scalar_private_storage_v1(
        actual
            .checked_sub(bytes)
            .ok_or_else(|| resource(Resource::Accounting))?,
    )?;
    charge(facts, count)?;
    output.resize(count, value);
    Ok(())
}

fn build_into<'a>(
    census: &mut ScalarPrivateBorrowsV1<'a>,
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    target: SemanticTargetDataLayoutV1,
    statements: usize,
    facts: &mut dyn ProjectedAssertionFactsV1,
) -> R<()> {
    facts.reserve_scalar_private_storage_v1(std::mem::size_of::<ScalarPrivateBorrowsV1<'_>>())?;
    allocate_into(
        &mut census.locals,
        function.locals().len(),
        Local::default(),
        facts,
    )?;
    allocate_into(
        &mut census.starts,
        function
            .blocks()
            .len()
            .checked_add(1)
            .ok_or_else(|| resource(Resource::Arithmetic))?,
        0usize,
        facts,
    )?;
    allocate_into(&mut census.reads, statements, None, facts)?;
    census.ledger = facts.helper_value_ledger_v1()?;
    let mut offset = 0usize;
    for (block, body) in function.blocks().iter().enumerate() {
        charge(facts, 3)?;
        census.starts[block] = offset;
        offset = offset
            .checked_add(body.statements().len())
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        for (statement, value) in body.statements().iter().enumerate() {
            if let Some(c) = candidate(
                types,
                function,
                target,
                block,
                statement,
                value.kind(),
                facts,
            )? {
                charge(facts, 6)?;
                if let Some(previous) = census.locals[c.alias].candidate {
                    census.locals[previous.root].blocked = true;
                    census.locals[c.alias].blocked = true;
                    census.locals[c.root].blocked = true;
                } else {
                    census.locals[c.alias].candidate = Some(c);
                }
                if census.locals[c.root].root_alias.is_some() {
                    census.locals[c.root].blocked = true;
                } else {
                    census.locals[c.root].root_alias = Some(c.alias);
                }
            }
        }
    }
    census.starts[function.blocks().len()] = offset;
    for (block, body) in function.blocks().iter().enumerate() {
        charge(facts, 2)?;
        let mut scan = Scan {
            census,
            facts,
            block,
            statement: None,
            allowed_place: None,
            allowed_destination: None,
        };
        for (statement, value) in body.statements().iter().enumerate() {
            scan.statement = Some(statement);
            scan.statement(value.kind())?;
        }
        scan.terminator(body.terminator().kind())?;
    }
    census.live_floor = facts.scalar_private_storage_v1()?;
    Ok(())
}

fn frame() -> R<usize> {
    // Logical typed source-policy rows, not native stack/RSS or allocator bytes.
    let rows=[
        size_of::<RetainedScalarBorrowsV1<'static>>(),
        size_of::<(Phase,Option<Source<'static>>,Option<Snapshot>,Option<Snapshot>,
            Option<ScalarPrivateBorrowsV1<'static>>,bool,bool,usize)>(),
        size_of::<(ScalarPrivateBorrowsV1<'static>,Option<ScalarPrivateBorrowsV1<'static>>,
            Vec<Local>,Vec<usize>,Vec<Option<Read<'static>>>,
            Source<'static>,&[SemanticTypeDeclV1],&SemanticFunctionDeclV1,SemanticTargetDataLayoutV1)>(),
        size_of::<(&mut RetainedScalarBorrowsV1<'static>,&RetainedScalarBorrowsV1<'static>,
            &mut Prep<'static,'static>,&Prep<'static,'static>,Source<'static>,Snapshot,Option<Snapshot>,bool)>(),
        size_of::<(Snapshot,Snapshot,Snapshot,Option<Snapshot>,Option<Source<'static>>,usize,usize,
            Option<usize>,Option<usize>,R<()>,R<usize>,Error,Resource,
            canonical_assertion_facts_v1::CanonicalAssertionErrorV1)>(),
        size_of::<(Meter<'static,'static,'static>,&mut Meter<'static,'static,'static>,
            &Meter<'static,'static,'static>,&mut dyn ProjectedAssertionFactsV1,
            &mut Prep<'static,'static>,&Prep<'static,'static>,Snapshot,
            (usize,CanonicalKernelIrWorkLedgerIdentityV1),R<(usize,CanonicalKernelIrWorkLedgerIdentityV1)>)>(),
        size_of::<(&mut ScalarPrivateBorrowsV1<'static>,&ScalarPrivateBorrowsV1<'static>,
            Option<&ScalarPrivateBorrowsV1<'static>>,Option<&mut ScalarPrivateBorrowsV1<'static>>,
            R<Option<&ScalarPrivateBorrowsV1<'static>>>,usize,Option<usize>)>(),
        size_of::<(std::iter::Enumerate<std::slice::Iter<'static,model::SemanticBasicBlockV1>>,
            (usize,&model::SemanticBasicBlockV1),usize,&model::SemanticBasicBlockV1,
            std::iter::Enumerate<std::slice::Iter<'static,model::SemanticStatementV1>>,
            (usize,&model::SemanticStatementV1),usize,&model::SemanticStatementV1,
            &SemanticStatementKindV1,usize,usize,Option<usize>,R<Option<Candidate>>,Option<Candidate>,bool)>(),
        size_of::<(Candidate,Candidate,Option<Candidate>,Option<Candidate>,Local,Local,
            Option<usize>,usize,usize,usize,usize,bool,bool,bool,bool)>(),
        // Three concrete instantiations of the unchanged allocation schedule.
        size_of::<(&mut Vec<Local>,Local,Local,usize,usize,usize,usize,
            Option<usize>,R<()>,std::result::Result<(),std::collections::TryReserveError>,
            std::collections::TryReserveError,Error,Resource,
            canonical_assertion_facts_v1::CanonicalAssertionErrorV1)>(),
        size_of::<(&mut Vec<usize>,usize,usize,usize,usize,usize,usize,
            Option<usize>,R<()>,std::result::Result<(),std::collections::TryReserveError>,
            std::collections::TryReserveError,Error,Resource,
            canonical_assertion_facts_v1::CanonicalAssertionErrorV1)>(),
        size_of::<(&mut Vec<Option<Read<'static>>>,Option<Read<'static>>,Option<Read<'static>>,
            usize,usize,usize,usize,Option<usize>,R<()>,
            std::result::Result<(),std::collections::TryReserveError>,std::collections::TryReserveError,
            Error,Resource,canonical_assertion_facts_v1::CanonicalAssertionErrorV1)>(),
        // Candidate classification parameters, borrowed enum payloads and getters.
        size_of::<(&[SemanticTypeDeclV1],&SemanticFunctionDeclV1,SemanticTargetDataLayoutV1,
            SemanticTargetDataLayoutV1,SemanticTargetArchitectureV1,
            usize,usize,&SemanticStatementKindV1,&SemanticAssignmentV1,&SemanticRvalueV1,
            &SemanticRvalueKindV1,&SemanticPlaceV1,&SemanticPlaceV1,usize,usize,
            &model::SemanticLocalDeclV1,&model::SemanticLocalDeclV1,
            Option<&model::SemanticLocalDeclV1>,Option<&model::SemanticLocalDeclV1>)>(),
        size_of::<(&SemanticTypeDeclV1,&SemanticTypeDeclV1,Option<&SemanticTypeDeclV1>,
            &SemanticTypeShapeV1,&SemanticTypeShapeV1,&u16,u16,u16,u64,u64,Option<u64>,
            &model::SemanticTypeLayoutV1,&model::SemanticPointerTypeV1,
            SemanticPointerKindV1,SemanticMutabilityV1,SemanticPointerMetadataV1,u32,u16,
            SemanticTypeIdV1,SemanticTypeIdV1,SemanticLocalRoleV1,SemanticLocalRoleV1,
            SemanticLocalIdV1,SemanticLocalIdV1,u32,u32,usize,usize,Candidate,Option<Candidate>,R<Option<Candidate>>)>(),
        size_of::<(Scan<'static,'static,'static>,&mut Scan<'static,'static,'static>,
            &Scan<'static,'static,'static>,&mut ScalarPrivateBorrowsV1<'static>,
            &mut dyn ProjectedAssertionFactsV1,usize,Option<usize>,
            Option<&SemanticPlaceV1>,Option<&SemanticPlaceV1>)>(),
        // Scan local/place, exact pointer predicates and Copy ID/projection getters.
        size_of::<(&mut Local,Option<&mut Local>,&Local,Option<&Local>,Local,Option<Local>,
            SemanticLocalIdV1,SemanticLocalIdV1,u32,usize,&SemanticPlaceV1,
            &model::SemanticLocalDeclV1,Option<&model::SemanticLocalDeclV1>,
            &SemanticTypeDeclV1,Option<&SemanticTypeDeclV1>,SemanticTypeIdV1,SemanticTypeIdV1,
            bool,bool,Option<&SemanticPlaceV1>,&SemanticPlaceV1,R<()>)>(),
        size_of::<(std::slice::Iter<'static,model::SemanticProjectionV1>,&model::SemanticProjectionV1,
            model::SemanticProjectionV1,model::SemanticProjectionV1,SemanticProjectionKindV1,
            SemanticTypeIdV1,SemanticTypeIdV1,SemanticLocalIdV1,SemanticLocalIdV1,u32,usize,
            &[model::SemanticProjectionV1],&SemanticProjectionKindV1)>(),
        // Scan operand/value and the exact closure capture passed to try_visit_operands.
        size_of::<(&SemanticOperandV1,&SemanticPlaceV1,&model::SemanticConstantV1,
            &SemanticRvalueV1,&SemanticRvalueKindV1,SemanticTypeIdV1,SemanticTypeIdV1,u32,usize,
            &mut Scan<'static,'static,'static>,&mut &mut Scan<'static,'static,'static>,
            &SemanticOperandV1,&SemanticOperandV1,
            &model::SemanticCheckedBinaryRvalueV1,&model::SemanticUncheckedBinaryRvalueV1,
            &model::SemanticAggregateRvalueV1,std::slice::Iter<'static,SemanticOperandV1>,
            &SemanticPlaceV1,&model::SemanticMemoryLoadV1,R<()>)>(),
        // Scan initialize/borrow/read/lifetime including closure aliases and copied states.
        size_of::<(&SemanticAssignmentV1,&SemanticPlaceV1,&SemanticPlaceV1,
            usize,usize,usize,Local,Local,Option<Local>,Option<Candidate>,Candidate,
            &mut Local,Option<usize>,Option<usize>,bool,bool,Read<'static>,Option<Read<'static>>,
            SemanticTypeIdV1,SemanticTypeIdV1,SemanticTypeIdV1,SemanticLocalIdV1,u32,
            &Scan<'static,'static,'static>,&Local,&model::SemanticProjectionV1,
            model::SemanticProjectionV1,SemanticProjectionKindV1,SemanticVolatilityV1,
            Option<SemanticAtomicAccessV1>,&model::SemanticMemoryLoadV1)>(),
        // Scan statement/message/terminator actual matched payload carriers.
        size_of::<(&SemanticStatementKindV1,&SemanticAssignmentV1,&model::SemanticMemoryStoreV1,
            &model::SemanticAtomicRmwV1,&model::SemanticAtomicCompareExchangeV1,
            &SemanticPlaceV1,&SemanticLocalIdV1,SemanticLocalIdV1,&SemanticOperandV1,
            &SemanticAssertMessageV1,&SemanticOperandV1,&SemanticOperandV1,
            &SemanticTerminatorKindV1,&model::SemanticDirectCallV1,
            &model::SemanticDirectTailCallV1,&model::SemanticCallDestinationV1,
            Option<&model::SemanticCallDestinationV1>,std::slice::Iter<'static,SemanticOperandV1>,
            &SemanticOperandV1,R<()>)>(),
        // Complete unchanged resolve parameters and every local/conversion carrier.
        size_of::<(&ScalarPrivateBorrowsV1<'static>,&SemanticFunctionDeclV1,
            &[SemanticTypeDeclV1],SemanticTargetDataLayoutV1,Site,&SemanticPlaceV1,
            AccessKindAttr,Option<SemanticAtomicAccessV1>,Option<LocalAllocationProvenanceV1>,
            &mut dyn ProjectedAssertionFactsV1,usize,usize,usize,usize,
            Option<&usize>,&usize,Option<usize>,Read<'static>,Option<Read<'static>>,Local,Local,
            Candidate,Option<Candidate>,SemanticLocalIdV1,SemanticLocalIdV1,u32,
            std::result::Result<u32,std::num::TryFromIntError>,std::num::TryFromIntError,
            LocalAllocationProvenanceV1,Option<SemanticLocalIdV1>,R<Option<SemanticLocalIdV1>>,
            R<usize>,R<(usize,CanonicalKernelIrWorkLedgerIdentityV1)>)>(),
        size_of::<(&mut Meter<'static,'static,'static>,usize,usize,bool,SemanticBlockIdV1,
            R<Option<u64>>,R<bool>,R<canonical_assertion_facts_v1::ProjectedAssertionConditionV1>,Error)>(),
        crate::production_ranked_projection_v1::bf16_nominal_preparation_resources_v1::retained_custody_snapshot_frame_v1(),
        size_of::<(&[model::SemanticLocalDeclV1],&[model::SemanticBasicBlockV1],
            &[model::SemanticStatementV1],&[SemanticOperandV1],&model::SemanticTerminatorV1,
            &Vec<Local>,&Vec<usize>,&Vec<Option<Read<'static>>>,&[Local],&[usize],&[Option<Read<'static>>])>(),
        size_of::<([usize;25],std::array::IntoIter<usize,25>,usize,usize,Option<usize>,R<usize>,Error)>(),
    ];
    rows.into_iter().try_fold(0usize, |sum, row| {
        sum.checked_add(row).ok_or_else(arithmetic)
    })
}
#[cfg(test)]
#[path = "retained_scalar_borrow_v1_tests.rs"]
mod tests;
