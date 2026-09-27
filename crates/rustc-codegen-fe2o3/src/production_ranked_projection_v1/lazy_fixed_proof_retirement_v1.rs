//! Private terminal payload transfer, not an authentic producer/factory token.
//! The caller retains the inert slot through its ORIGINAL returning postflight
//! or outer panic custody/refund path, then drops it before refunding its credits.
#![allow(dead_code)]
use super::*;
use crate::production_ranked_projection_v1::assertion_resources_v1::RetiredAssertionCacheV1;
use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

type Panic = Box<dyn Any + Send>;
type Caught<R> = std::result::Result<Result<R>, Panic>;

struct SideOwners {
    graph: Option<ProjectedLoopCfgV1>,
    definition_counts: Option<Vec<u8>>,
    block_definitions: Option<Vec<Vec<usize>>>,
    address_escaped: Option<Vec<bool>>,
    assignments: Option<Vec<Option<ScalarAssignmentSiteV1>>>,
    statement_definitions: Option<StatementDefinitionIndexV1>,
}
/// Opaque and lifetime-free. No query, mutable accessor, Clone, rehydration,
/// source handle, resource adapter, ledger identity, or refund operation.
pub(in crate::production_ranked_projection_v1) struct RetiredLazyProofPayloadsV1 {
    checked: Vec<Vec<usize>>,
    dominance: Option<RetiredAssertionCacheV1>,
    zero_exclusion: Option<RetiredAssertionCacheV1>,
    side: SideOwners,
}
struct EmptySlot<'s> {
    slot: &'s mut Option<RetiredLazyProofPayloadsV1>,
}
impl EmptySlot<'_> {
    // The only constructor checked None while acquiring this exclusive loan.
    // Assignment does not replace/drop an occupied payload or run user code.
    fn install(self, payload: RetiredLazyProofPayloadsV1) {
        *self.slot = Some(payload);
    }
}
fn empty_side() -> SideOwners {
    SideOwners {
        graph: None,
        definition_counts: None,
        block_definitions: None,
        address_escaped: None,
        assignments: None,
        statement_definitions: None,
    }
}
fn empty_payload() -> RetiredLazyProofPayloadsV1 {
    RetiredLazyProofPayloadsV1 {
        checked: Vec::new(),
        dominance: None,
        zero_exclusion: None,
        side: empty_side(),
    }
}
fn retire_cache(cache: Option<AssertionCacheV1<'_>>) -> Option<RetiredAssertionCacheV1> {
    match cache {
        Some(cache) => Some(cache.retire_payload_v1()),
        None => None,
    }
}
fn retire_table<T>(table: AssertionTableV1<'_, T>) -> Option<Vec<T>> {
    match table {
        AssertionTableV1::Owned(rows) => Some(rows),
        AssertionTableV1::Borrowed(_) => None,
    }
}
fn retire_ready(session: PreparedFixedGuardSessionV1<'_>) -> RetiredLazyProofPayloadsV1 {
    let PreparedFixedGuardSessionV1 {
        proof,
        original_ledger: _,
        failed: _,
    } = session;
    retire_proof(proof)
}
fn retire_proof(proof: SemanticAssertProofsV1<'_>) -> RetiredLazyProofPayloadsV1 {
    let SemanticAssertProofsV1 {
        types: _,
        function: _,
        graph,
        definition_counts,
        block_definitions,
        address_escaped,
        assignments,
        checked_assertion_blocks,
        statement_definitions,
        dominance,
        zero_exclusion,
        work: _,
        resources,
    } = proof;
    let graph = match graph {
        AssertionGraphV1::Owned(graph) => Some(graph),
        AssertionGraphV1::Borrowed(_) => None,
    };
    let retained = RetiredLazyProofPayloadsV1 {
        checked: checked_assertion_blocks,
        dominance: Some(dominance.retire_payload_v1()),
        zero_exclusion: Some(zero_exclusion.retire_payload_v1()),
        side: SideOwners {
            graph,
            definition_counts: retire_table(definition_counts),
            block_definitions: retire_table(block_definitions),
            address_escaped: retire_table(address_escaped),
            assignments: retire_table(assignments),
            statement_definitions,
        },
    };
    // This handle has no custom Drop. Only its exclusive resource loan ends;
    // every independently owned proof buffer has already moved to retained.
    drop(resources);
    retained
}
fn retire_owner(owner: LazyFixedProofOwnerV1<'_, '_, '_>) -> RetiredLazyProofPayloadsV1 {
    let LazyFixedProofOwnerV1 {
        input: _,
        original_ledger: _,
        phase,
        next_block: _,
        failed: _,
    } = owner;
    match phase {
        Phase::Pending(_) | Phase::Unavailable => empty_payload(),
        Phase::Building(Building {
            resources,
            checked,
            dominance,
            zero_exclusion,
        }) => {
            let retained = RetiredLazyProofPayloadsV1 {
                checked,
                dominance: retire_cache(dominance),
                zero_exclusion: retire_cache(zero_exclusion),
                side: empty_side(),
            };
            drop(resources);
            retained
        }
        Phase::Ready(session) => retire_ready(session),
    }
}
/// Each row is a selected source vertex, using the existing typed frame
/// convention (locals + 2 return + 2 Result<return>). This is not a machine
/// stack/RSS bound. No fixed slack covers an omitted source vertex.
fn transfer_rows<F, R>() -> Result<[usize; 21]> {
    type O = LazyFixedProofOwnerV1<'static, 'static, 'static>;
    type P = RetiredLazyProofPayloadsV1;
    Ok([
        // Public source-view entry (the fixture entry has its own row below).
        frame::<R>(size_of::<(
            &NominalRootCfgSourceV1<'static>,
            &mut Prep<'static, 'static>,
            &mut Option<P>,
            F,
            FixedGuardInputsV1<'static, 'static>,
        )>())?,
        // Input entry, exclusive slot, original owner, full catch/result/panic.
        frame::<R>(size_of::<(
            FixedGuardInputsV1<'static, 'static>,
            &mut Prep<'static, 'static>,
            &mut Option<P>,
            F,
            usize,
            EmptySlot<'static>,
            Result<O>,
            O,
            Caught<R>,
            P,
            Result<R>,
            Panic,
        )>())?,
        // The catch closure captures precisely moved F and &mut original O.
        frame::<Result<R>>(size_of::<(F, &mut O, AssertUnwindSafe<(F, &mut O)>)>())?,
        // catch_unwind/resume transfer representations; no std-internal claim.
        frame::<Caught<R>>(size_of::<(AssertUnwindSafe<(F, &mut O)>, Caught<R>, Panic)>())?,
        frame::<P>(size_of::<(
            O,
            Phase<'static, 'static, 'static>,
            Building<'static>,
            PreparedFixedGuardSessionV1<'static>,
            AssertionResourcesV1<'static>,
            P,
        )>())?,
        frame::<P>(size_of::<(
            PreparedFixedGuardSessionV1<'static>,
            SemanticAssertProofsV1<'static>,
            FixedGuardLedgerV1,
            bool,
        )>())?,
        frame::<P>(size_of::<(
            SemanticAssertProofsV1<'static>,
            AssertionGraphV1<'static>,
            Option<ProjectedLoopCfgV1>,
            AssertionTableV1<'static, u8>,
            AssertionTableV1<'static, Vec<usize>>,
            AssertionTableV1<'static, bool>,
            AssertionTableV1<'static, Option<ScalarAssignmentSiteV1>>,
            Vec<Vec<usize>>,
            Option<StatementDefinitionIndexV1>,
            AssertionCacheV1<'static>,
            AssertionCacheV1<'static>,
            AssertionResourcesV1<'static>,
            P,
        )>())?,
        frame::<Option<Vec<u8>>>(size_of::<(AssertionTableV1<'static, u8>, Vec<u8>)>())?,
        frame::<Option<Vec<Vec<usize>>>>(size_of::<(
            AssertionTableV1<'static, Vec<usize>>,
            Vec<Vec<usize>>,
        )>())?,
        frame::<Option<Vec<bool>>>(size_of::<(AssertionTableV1<'static, bool>, Vec<bool>)>())?,
        frame::<Option<Vec<Option<ScalarAssignmentSiteV1>>>>(size_of::<(
            AssertionTableV1<'static, Option<ScalarAssignmentSiteV1>>,
            Vec<Option<ScalarAssignmentSiteV1>>,
        )>())?,
        // Two concrete cache moves are admitted. Cache internals are measured
        // by their own defining module, never guessed through a public header.
        frame::<Option<RetiredAssertionCacheV1>>(size_of::<(
            Option<AssertionCacheV1<'static>>,
            AssertionCacheV1<'static>,
        )>())?
        .checked_add(AssertionCacheV1::retirement_frame_v1().ok_or_else(arithmetic)?)
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(arithmetic)?,
        frame::<()>(size_of::<(EmptySlot<'static>, P, Option<P>)>())?,
        frame::<SideOwners>(size_of::<SideOwners>())?,
        frame::<P>(size_of::<(P, SideOwners, Vec<Vec<usize>>)>())?,
        // Closed component fixture entry; conservatively also paid in prod.
        frame::<R>(size_of::<(
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &ProjectedLoopCfgV1,
            &RichNominalSourceTablesV1<'static>,
            &mut Prep<'static, 'static>,
            &mut Option<P>,
            F,
            FixedGuardInputsV1<'static, 'static>,
        )>())?,
        frame::<[usize; 21]>(size_of::<([usize; 21], Option<usize>)>())?,
        frame::<usize>(size_of::<(Result<[usize; 21]>, [usize; 21], &[usize])>())?,
        frame::<usize>(size_of::<(&[usize], usize, &usize, Option<usize>)>())?,
        frame::<Error>(size_of::<Resource>())?,
        frame::<Error>(size_of::<Resource>())?,
    ])
}
fn transfer_frame<F, R>() -> Result<usize> {
    sum(&transfer_rows::<F, R>()?)
}
fn with_input<'a, 'b, 'w, F, R: Copy + 'static>(
    input: FixedGuardInputsV1<'a, 'a>,
    resources: &'a mut Prep<'b, 'w>,
    slot: &mut Option<RetiredLazyProofPayloadsV1>,
    run: F,
) -> Result<R>
where
    F: FnOnce(&mut LazyFixedProofOwnerV1<'a, 'b, 'w>) -> Result<R>,
{
    // Occupied storage stays physically unchanged. No work/owner admission
    // and no callback occurs before this exact None check.
    if slot.is_some() {
        return Err(accounting());
    }
    if !resources.is_metered() || resources.has_denial() {
        return Err(accounting());
    }
    let bytes = transfer_frame::<F, R>()?;
    resources.work(bytes)?;
    resources.reserve_storage(bytes)?;
    let reserved = EmptySlot { slot };
    // Constructor failure has no Vec/cache payload; its accepted credits still
    // belong to the original caller. No replacement adapter or retry is made.
    let mut owner = LazyFixedProofOwnerV1::new(input, resources)?;
    let outcome = catch_unwind(AssertUnwindSafe(|| run(&mut owner)));
    // No fallible check, allocation, arbitrary closure or denial check here.
    reserved.install(retire_owner(owner));
    match outcome {
        Ok(result) => result,
        // Original panic object/priority reaches the EXISTING outer catch.
        // In particular do not force a returning recipe postcheck on this path.
        Err(payload) => resume_unwind(payload),
    }
}
pub(in crate::production_ranked_projection_v1) fn with_retired_lazy_proof_v1<
    'a,
    'b,
    'w,
    F,
    R: Copy + 'static,
>(
    view: &'a NominalRootCfgSourceV1<'_>,
    resources: &'a mut Prep<'b, 'w>,
    slot: &mut Option<RetiredLazyProofPayloadsV1>,
    run: F,
) -> Result<R>
where
    F: FnOnce(&mut LazyFixedProofOwnerV1<'a, 'b, 'w>) -> Result<R>,
{
    with_input(
        FixedGuardInputsV1 {
            types: view.types(),
            function: view.function(),
            graph: view.graph(),
            rich: view.source_tables().rich(),
        },
        resources,
        slot,
        run,
    )
}

#[cfg(test)]
pub(in crate::production_ranked_projection_v1) fn with_fixture_v1<
    'a,
    'b,
    'w,
    F,
    R: Copy + 'static,
>(
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    graph: &'a ProjectedLoopCfgV1,
    rich: &'a RichNominalSourceTablesV1<'a>,
    resources: &'a mut Prep<'b, 'w>,
    slot: &mut Option<RetiredLazyProofPayloadsV1>,
    run: F,
) -> Result<R>
where
    F: FnOnce(&mut LazyFixedProofOwnerV1<'a, 'b, 'w>) -> Result<R>,
{
    with_input(
        FixedGuardInputsV1 {
            types,
            function,
            graph,
            rich,
        },
        resources,
        slot,
        run,
    )
}
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::production_ranked_projection_v1) struct Snapshot {
    pub checked: (usize, usize, usize),
    pub first: Option<(usize, usize, usize)>,
    pub dominance: Option<(u8, usize, usize, usize)>,
    pub zero: Option<(u8, usize, usize, usize)>,
    pub side: [bool; 6],
}
#[cfg(test)]
impl RetiredLazyProofPayloadsV1 {
    pub(in crate::production_ranked_projection_v1) fn snapshot(&self) -> Snapshot {
        Snapshot {
            checked: (
                self.checked.as_ptr() as usize,
                self.checked.len(),
                self.checked.capacity(),
            ),
            first: self
                .checked
                .first()
                .map(|row| (row.as_ptr() as usize, row.len(), row.capacity())),
            dominance: self
                .dominance
                .as_ref()
                .map(RetiredAssertionCacheV1::snapshot_for_test),
            zero: self
                .zero_exclusion
                .as_ref()
                .map(RetiredAssertionCacheV1::snapshot_for_test),
            side: [
                self.side.graph.is_some(),
                self.side.definition_counts.is_some(),
                self.side.block_definitions.is_some(),
                self.side.address_escaped.is_some(),
                self.side.assignments.is_some(),
                self.side.statement_definitions.is_some(),
            ],
        }
    }
}
#[cfg(test)]
pub(in crate::production_ranked_projection_v1) fn live_snapshot(
    owner: &LazyFixedProofOwnerV1<'_, '_, '_>,
) -> Snapshot {
    let (checked, dominance, zero) = match &owner.phase {
        Phase::Building(b) => (&b.checked, b.dominance.as_ref(), b.zero_exclusion.as_ref()),
        Phase::Ready(s) => (
            &s.proof.checked_assertion_blocks,
            Some(&s.proof.dominance),
            Some(&s.proof.zero_exclusion),
        ),
        _ => return empty_payload().snapshot(),
    };
    Snapshot {
        checked: (checked.as_ptr() as usize, checked.len(), checked.capacity()),
        first: checked
            .first()
            .map(|row| (row.as_ptr() as usize, row.len(), row.capacity())),
        dominance: dominance.map(AssertionCacheV1::snapshot_for_retirement_test),
        zero: zero.map(AssertionCacheV1::snapshot_for_retirement_test),
        side: [false; 6],
    }
}
#[cfg(test)]
pub(in crate::production_ranked_projection_v1) fn seed_ready_for_test(
    owner: &mut LazyFixedProofOwnerV1<'_, '_, '_>,
) -> Result<()> {
    let Phase::Ready(session) = &mut owner.phase else {
        return Err(accounting());
    };
    // Synthetic payload control, not a source query or production branch.
    let resources = &mut session.proof.resources;
    resources.reserve_frame::<()>(size_of::<(
        &mut LazyFixedProofOwnerV1<'_, '_, '_>,
        &mut PreparedFixedGuardSessionV1<'_>,
        &mut AssertionResourcesV1<'_>,
        Option<&mut Vec<usize>>,
        &mut Vec<usize>,
        Result<()>,
    )>())?;
    let row = session
        .proof
        .checked_assertion_blocks
        .first_mut()
        .ok_or_else(accounting)?;
    resources.push_vec(row, 123, "retirement test row")?;
    session.proof.dominance.insert((31, 47), true, resources)?;
    session
        .proof
        .zero_exclusion
        .insert((53, 59), false, resources)
}
#[cfg(test)]
pub(in crate::production_ranked_projection_v1) fn retire_legacy_proof_for_test(
    proof: SemanticAssertProofsV1<'_>,
) -> RetiredLazyProofPayloadsV1 {
    // Deliberate unmetered legacy fixture only, never a strict factory path.
    retire_proof(proof)
}
#[cfg(test)]
#[path = "lazy_fixed_proof_retirement_frame_v1_tests.rs"]
mod frame_controls;
