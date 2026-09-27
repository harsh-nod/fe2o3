//! Independent ORIGINAL constructor retention component, cfg(test) only.
//! No query driver, lazy owner/visit oracle, genuine Fixed acceptance or F2.
use super::*;
use crate::production_ranked_projection_v1::assertion_resources_v1::{
    RetiredAssertionCacheV1,
    original_nested_retention::{self as nested, Cuts, Stage},
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::{any::Any, mem::size_of};
type Error = ProductionRankedProjectionErrorV1;
type R<T> = std::result::Result<T, Error>;
type Prep<'b, 'w> = PreparationResourcesV1<'b, 'w>;
type Rich<'a> = super::super::bf16_nominal_source_preparation_v1::RichNominalSourceTablesV1<'a>;
type Panic = Box<dyn Any + Send>;
type Triple = (usize, usize, usize);
pub(in crate::production_ranked_projection_v1) use nested::{
    Cuts as ConstructorCuts, Stage as ConstructorStage,
};

#[path = "original_fixed_constructor_debit_oracle_v1_tests.rs"]
mod debit_oracle;
pub(in crate::production_ranked_projection_v1) use debit_oracle::{Debit, expected_debits};

struct Building<'a> {
    resources: AssertionResourcesV1<'a>,
    checked: Vec<Vec<usize>>,
    dominance: Option<AssertionCacheV1<'a>>,
    zero: Option<AssertionCacheV1<'a>>,
}
#[allow(clippy::large_enum_variant)]
enum Phase<'a, 'b, 'w> {
    Pending(Option<&'a mut Prep<'b, 'w>>),
    Building(Building<'a>),
    Ready(PreparedFixedGuardSessionV1<'a>),
    Unavailable,
}
struct OriginalFixedOracleOwnerV1<'a, 'b, 'w> {
    input: FixedGuardInputsV1<'a, 'a>,
    ledger: Option<FixedGuardLedgerV1>,
    phase: Phase<'a, 'b, 'w>,
    attempted: bool,
}
struct Side {
    graph: Option<ProjectedLoopCfgV1>,
    counts: Option<Vec<u8>>,
    blocks: Option<Vec<Vec<usize>>>,
    escaped: Option<Vec<bool>>,
    assignments: Option<Vec<Option<ScalarAssignmentSiteV1>>>,
    statements: Option<StatementDefinitionIndexV1>,
}
/// Drop-only inert DATA: no source/resource/ledger handle or reverse path.
pub(in crate::production_ranked_projection_v1) struct RetiredOriginalFixedOracleV1 {
    checked: Vec<Vec<usize>>,
    dominance: Option<RetiredAssertionCacheV1>,
    zero: Option<RetiredAssertionCacheV1>,
    side: Side,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) struct Snapshot {
    pub phase: u8,
    pub checked: Triple,
    pub rows: [Triple; 32],
    pub complete_rows: bool,
    pub dominance: Option<(u8, usize, usize, usize)>,
    pub zero: Option<(u8, usize, usize, usize)>,
    pub side: [bool; 6],
}
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::production_ranked_projection_v1) struct Witness {
    pub admitted: bool,
    pub installed: bool,
    pub before: Option<Snapshot>,
    pub after: Option<Snapshot>,
    pub prefix: usize,
}
fn empty_side() -> Side {
    Side {
        graph: None,
        counts: None,
        blocks: None,
        escaped: None,
        assignments: None,
        statements: None,
    }
}
fn empty_retired() -> RetiredOriginalFixedOracleV1 {
    RetiredOriginalFixedOracleV1 {
        checked: Vec::new(),
        dominance: None,
        zero: None,
        side: empty_side(),
    }
}
fn rows_snapshot(rows: &Vec<Vec<usize>>) -> (Triple, [Triple; 32], bool) {
    let mut actual = [(0, 0, 0); 32];
    for (output, row) in actual.iter_mut().zip(rows.iter()) {
        *output = (row.as_ptr() as usize, row.len(), row.capacity());
    }
    (
        (rows.as_ptr() as usize, rows.len(), rows.capacity()),
        actual,
        rows.len() <= actual.len(),
    )
}
fn side_snapshot(side: &Side) -> [bool; 6] {
    [
        side.graph.is_some(),
        side.counts.is_some(),
        side.blocks.is_some(),
        side.escaped.is_some(),
        side.assignments.is_some(),
        side.statements.is_some(),
    ]
}
impl RetiredOriginalFixedOracleV1 {
    pub fn snapshot(&self, phase: u8) -> Snapshot {
        let (checked, rows, complete_rows) = rows_snapshot(&self.checked);
        Snapshot {
            phase,
            checked,
            rows,
            complete_rows,
            dominance: self
                .dominance
                .as_ref()
                .map(RetiredAssertionCacheV1::snapshot_for_test),
            zero: self
                .zero
                .as_ref()
                .map(RetiredAssertionCacheV1::snapshot_for_test),
            side: side_snapshot(&self.side),
        }
    }
}
fn proof_snapshot(proof: &SemanticAssertProofsV1<'_>, phase: u8) -> Snapshot {
    let (checked, rows, complete_rows) = rows_snapshot(&proof.checked_assertion_blocks);
    Snapshot {
        phase,
        checked,
        rows,
        complete_rows,
        dominance: Some(proof.dominance.snapshot_for_retirement_test()),
        zero: Some(proof.zero_exclusion.snapshot_for_retirement_test()),
        side: [
            matches!(proof.graph, AssertionGraphV1::Owned(_)),
            matches!(proof.definition_counts, AssertionTableV1::Owned(_)),
            matches!(proof.block_definitions, AssertionTableV1::Owned(_)),
            matches!(proof.address_escaped, AssertionTableV1::Owned(_)),
            matches!(proof.assignments, AssertionTableV1::Owned(_)),
            proof.statement_definitions.is_some(),
        ],
    }
}
impl<'a, 'b, 'w> OriginalFixedOracleOwnerV1<'a, 'b, 'w> {
    fn new(input: FixedGuardInputsV1<'a, 'a>, resources: &'a mut Prep<'b, 'w>) -> Self {
        // Original identity capture is BEFORE strict(), never reconstructed.
        let ledger = resources.original_ledger_v1();
        Self {
            input,
            ledger,
            phase: Phase::Pending(Some(resources)),
            attempted: false,
        }
    }
    fn prepare(&mut self, cuts: &mut Cuts) -> R<()> {
        if self.attempted {
            return Err(assertion_resource_accounting_v1());
        }
        self.attempted = true;
        let resources = match &mut self.phase {
            Phase::Pending(slot) => slot.take().ok_or_else(assertion_resource_accounting_v1)?,
            _ => return Err(assertion_resource_accounting_v1()),
        };
        self.phase = Phase::Unavailable;
        let strict = AssertionResourcesV1::strict(resources)?;
        // Attach the ONE strict handle before any checkpoint or proof admission.
        self.phase = Phase::Building(Building {
            resources: strict,
            checked: Vec::new(),
            dominance: None,
            zero: None,
        });
        cuts.checkpoint(Stage::Strict);
        let Phase::Building(building) = &mut self.phase else {
            unreachable!()
        };
        let ledger = initialize_original(self.input, self.ledger, building, cuts)?;
        let Phase::Building(building) = std::mem::replace(&mut self.phase, Phase::Unavailable)
        else {
            unreachable!()
        };
        // No fallible/callback/allocation work in this private successful move.
        self.phase = Phase::Ready(finish_original(self.input, ledger, building));
        cuts.checkpoint(Stage::Ready);
        Ok(())
    }
    fn snapshot(&self) -> Snapshot {
        match &self.phase {
            Phase::Pending(_) | Phase::Unavailable => empty_retired().snapshot(0),
            Phase::Building(building) => {
                let (checked, rows, complete_rows) = rows_snapshot(&building.checked);
                Snapshot {
                    phase: 1,
                    checked,
                    rows,
                    complete_rows,
                    dominance: building
                        .dominance
                        .as_ref()
                        .map(AssertionCacheV1::snapshot_for_retirement_test),
                    zero: building
                        .zero
                        .as_ref()
                        .map(AssertionCacheV1::snapshot_for_retirement_test),
                    side: [false; 6],
                }
            }
            Phase::Ready(session) => proof_snapshot(&session.proof, 2),
        }
    }
}

fn initialize_original<'a>(
    input: FixedGuardInputsV1<'a, '_>,
    ledger: Option<FixedGuardLedgerV1>,
    building: &mut Building<'a>,
    cuts: &mut Cuts,
) -> R<FixedGuardLedgerV1> {
    // Identity capture and strict creation occur in new/prepare, in original order.
    reserve_fixed_guard_frames_v1(&mut building.resources)?;
    cuts.checkpoint(Stage::FixedFrames);
    building.resources.extra_work(1)?;
    cuts.checkpoint(Stage::FirstWork);
    let ledger = ledger.ok_or_else(assertion_resource_accounting_v1)?;
    cuts.checkpoint(Stage::Ledger);
    if !std::ptr::eq(input.function, input.rich.function())
        || !input.rich.belongs_to_original_ledger_v1(ledger)
    {
        return Err(fixed_source_refusal_v1());
    }
    cuts.checkpoint(Stage::Source);
    reserve_assertion_evaluator_frames_v1(&mut building.resources)?;
    cuts.checkpoint(Stage::EvaluatorFrames);
    initialize_borrowed_original(input, building, cuts)?;
    Ok(ledger)
}

fn initialize_borrowed_original<'a>(
    input: FixedGuardInputsV1<'a, '_>,
    building: &mut Building<'a>,
    cuts: &mut Cuts,
) -> R<()> {
    let function = input.function;
    let graph = input.graph;
    let definition_counts = input.rich.scalar_counts();
    let block_definitions = input.rich.scalar_blocks();
    let address_escaped = input.rich.address_escaped();
    let assignments = input.rich.scalar_assignments();
    let resources = &mut building.resources;
    resources.reserve_frame::<SemanticAssertProofsV1<'_>>(0)?;
    cuts.checkpoint(Stage::ProofFrame);
    resources.extra_work(64)?;
    cuts.checkpoint(Stage::ProofWork);
    let locals = function.locals().len();
    let blocks = function.blocks().len();
    if !resources.is_strict()
        || definition_counts.len() != locals
        || address_escaped.len() != locals
        || assignments.len() != locals
        || block_definitions.len() != blocks
        || graph.successors.len() != blocks
        || graph.predecessors.len() != blocks
        || graph.reachable.len() != blocks
        || graph.entry != function.entry().index() as usize
    {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "prepared assertion source tables differ from the exact function",
        ));
    }
    cuts.checkpoint(Stage::Shape);
    checked_index_into_original(function, resources, &mut building.checked, cuts)?;
    building.dominance = Some(AssertionCacheV1::new(resources)?);
    cuts.checkpoint(Stage::Dominance);
    building.zero = Some(AssertionCacheV1::new(resources)?);
    cuts.checkpoint(Stage::Zero);
    Ok(())
}

fn checked_index_into_original<'a>(
    function: &SemanticFunctionDeclV1,
    resources: &mut AssertionResourcesV1<'a>,
    checked_assertion_blocks: &mut Vec<Vec<usize>>,
    cuts: &mut Cuts,
) -> R<()> {
    if resources.is_strict() {
        nested::nested_into_original(
            resources,
            checked_assertion_blocks,
            function.locals().len(),
            cuts,
        )?;
    } else {
        *checked_assertion_blocks = vec![Vec::new(); function.locals().len()];
    }
    resources.extra_work(function.blocks().len())?;
    cuts.checkpoint(Stage::BlockWork);
    for (block_index, block) in function.blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Assert { condition, .. } = block.terminator().kind() else {
            continue;
        };
        let Some(local) = tuple_field_operand_local_v1(condition, 1) else {
            continue;
        };
        let Some(blocks) = checked_assertion_blocks.get_mut(local.index() as usize) else {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a checked arithmetic assertion is outside the semantic local table",
            ));
        };
        resources.push_vec(
            blocks,
            block_index,
            "checked arithmetic assertion storage cannot be reserved",
        )?;
        cuts.checkpoint(Stage::RowPush);
    }
    Ok(())
}

fn finish_original<'a>(
    input: FixedGuardInputsV1<'a, '_>,
    ledger: FixedGuardLedgerV1,
    building: Building<'a>,
) -> PreparedFixedGuardSessionV1<'a> {
    let proof = SemanticAssertProofsV1 {
        types: input.types,
        function: input.function,
        graph: AssertionGraphV1::Borrowed(input.graph),
        definition_counts: AssertionTableV1::Borrowed(input.rich.scalar_counts()),
        block_definitions: AssertionTableV1::Borrowed(input.rich.scalar_blocks()),
        address_escaped: AssertionTableV1::Borrowed(input.rich.address_escaped()),
        assignments: AssertionTableV1::Borrowed(input.rich.scalar_assignments()),
        checked_assertion_blocks: building.checked,
        statement_definitions: None,
        dominance: building
            .dominance
            .expect("original first cache successfully installed"),
        zero_exclusion: building
            .zero
            .expect("original second cache successfully installed"),
        work: 0,
        resources: building.resources,
    };
    PreparedFixedGuardSessionV1 {
        proof,
        original_ledger: ledger,
        failed: false,
    }
}
fn retire_table<T>(table: AssertionTableV1<'_, T>) -> Option<Vec<T>> {
    match table {
        AssertionTableV1::Owned(rows) => Some(rows),
        AssertionTableV1::Borrowed(_) => None,
    }
}
fn retire_cache(cache: Option<AssertionCacheV1<'_>>) -> Option<RetiredAssertionCacheV1> {
    cache.map(AssertionCacheV1::retire_payload_v1)
}
fn retire_proof(proof: SemanticAssertProofsV1<'_>) -> RetiredOriginalFixedOracleV1 {
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
    let retained = RetiredOriginalFixedOracleV1 {
        checked: checked_assertion_blocks,
        dominance: Some(dominance.retire_payload_v1()),
        zero: Some(zero_exclusion.retire_payload_v1()),
        side: Side {
            graph: match graph {
                AssertionGraphV1::Owned(g) => Some(g),
                AssertionGraphV1::Borrowed(_) => None,
            },
            counts: retire_table(definition_counts),
            blocks: retire_table(block_definitions),
            escaped: retire_table(address_escaped),
            assignments: retire_table(assignments),
            statements: statement_definitions,
        },
    };
    drop(resources); // only the resource loan ends; owned buffers already moved.
    retained
}
fn retire(owner: OriginalFixedOracleOwnerV1<'_, '_, '_>) -> RetiredOriginalFixedOracleV1 {
    match owner.phase {
        Phase::Pending(_) | Phase::Unavailable => empty_retired(),
        Phase::Building(Building {
            resources,
            checked,
            dominance,
            zero,
        }) => {
            let retained = RetiredOriginalFixedOracleV1 {
                checked,
                dominance: retire_cache(dominance),
                zero: retire_cache(zero),
                side: empty_side(),
            };
            drop(resources);
            retained
        }
        Phase::Ready(PreparedFixedGuardSessionV1 {
            proof,
            original_ledger: _,
            failed: _,
        }) => retire_proof(proof),
    }
}
struct EmptySlot<'s>(&'s mut Option<RetiredOriginalFixedOracleV1>);
impl EmptySlot<'_> {
    fn install(self, payload: RetiredOriginalFixedOracleV1) {
        *self.0 = Some(payload);
    }
}

fn admit(resources: &mut Prep<'_, '_>) -> R<usize> {
    if !resources.is_metered() || resources.has_denial() {
        return Err(assertion_resource_accounting_v1());
    }
    let bytes = added_frame()?;
    resources.work(bytes)?;
    resources.reserve_storage(bytes)?;
    Ok(bytes)
}
#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn retained_constructor<'a, 'b, 'w>(
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    graph: &'a ProjectedLoopCfgV1,
    rich: &'a Rich<'a>,
    resources: &'a mut Prep<'b, 'w>,
    slot: &mut Option<RetiredOriginalFixedOracleV1>,
    cuts: &mut Cuts,
    witness: &mut Witness,
    retry: bool,
) -> R<Snapshot> {
    if slot.is_some() {
        return Err(assertion_resource_accounting_v1());
    }
    let prefix = admit(resources)?;
    witness.admitted = true;
    witness.prefix = prefix;
    cuts.prepare_payload();
    let reserved = EmptySlot(slot);
    let input = FixedGuardInputsV1 {
        types,
        function,
        graph,
        rich,
    };
    let mut owner = OriginalFixedOracleOwnerV1::new(input, resources);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        owner.prepare(cuts)?;
        if retry {
            owner.prepare(cuts)?;
        }
        Ok(owner.snapshot())
    }));
    let before = owner.snapshot();
    witness.before = Some(before);
    // No admission, denial check, allocation or arbitrary callback in transfer.
    reserved.install(retire(owner));
    witness.installed = true;
    witness.after = slot.as_ref().map(|p| p.snapshot(before.phase));
    match outcome {
        Ok(result) => result,
        Err(payload) => resume_unwind(payload),
    }
}
#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn original_reference<'a>(
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    graph: &'a ProjectedLoopCfgV1,
    rich: &'a Rich<'a>,
    resources: &'a mut Prep<'_, '_>,
) -> R<Snapshot> {
    // Exactly the same explicit observational prefix, not an inferred residual.
    admit(resources)?;
    let session = new_fixed_guard_session_v1(
        FixedGuardInputsV1 {
            types,
            function,
            graph,
            rich,
        },
        resources,
    )?;
    let result = proof_snapshot(&session.proof, 2);
    drop(session);
    Ok(result)
}

// Conservative logical source policy; not a native stack or RSS measurement.
type CatchCaptures = (
    &'static mut OriginalFixedOracleOwnerV1<'static, 'static, 'static>,
    &'static mut &'static mut Cuts,
    &'static bool,
);
const ADDED_ROWS: usize = 38;
fn frame<T>(locals: usize) -> R<usize> {
    locals
        .checked_add(
            size_of::<T>()
                .checked_mul(2)
                .ok_or_else(assertion_resource_overflow_v1)?,
        )
        .and_then(|n| {
            size_of::<R<T>>()
                .checked_mul(2)
                .and_then(|r| n.checked_add(r))
        })
        .ok_or_else(assertion_resource_overflow_v1)
}
fn added_rows() -> R<[usize; ADDED_ROWS]> {
    Ok([
        // retained entry and exact source arguments
        frame::<Snapshot>(size_of::<(
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &ProjectedLoopCfgV1,
            &Rich<'static>,
            &mut Prep<'static, 'static>,
            &mut Option<RetiredOriginalFixedOracleV1>,
            &mut Cuts,
            &mut Witness,
            bool,
            usize,
            FixedGuardInputsV1<'static, 'static>,
            OriginalFixedOracleOwnerV1<'static, 'static, 'static>,
            EmptySlot<'static>,
            std::thread::Result<R<Snapshot>>,
            Snapshot,
            Panic,
        )>())?,
        // admit original Prep identity/denial and complete prefix
        frame::<usize>(size_of::<(
            &mut Prep<'static, 'static>,
            usize,
            R<usize>,
            R<()>,
            bool,
            Error,
        )>())?,
        // owner fresh capture before strict
        frame::<OriginalFixedOracleOwnerV1<'static, 'static, 'static>>(size_of::<(
            FixedGuardInputsV1<'static, 'static>,
            &mut Prep<'static, 'static>,
            Option<FixedGuardLedgerV1>,
            Phase<'static, 'static, 'static>,
            bool,
        )>())?,
        // prepare phase/strict/transitions and returned errors
        frame::<()>(size_of::<(
            &mut OriginalFixedOracleOwnerV1<'static, 'static, 'static>,
            &mut Cuts,
            &mut Phase<'static, 'static, 'static>,
            &mut Option<&mut Prep<'static, 'static>>,
            &mut Prep<'static, 'static>,
            AssertionResourcesV1<'static>,
            Building<'static>,
            &mut Building<'static>,
            FixedGuardLedgerV1,
            Phase<'static, 'static, 'static>,
            R<()>,
            Error,
        )>())?,
        // independent original fixed constructor with borrowed Building
        frame::<FixedGuardLedgerV1>(size_of::<(
            FixedGuardInputsV1<'static, 'static>,
            Option<FixedGuardLedgerV1>,
            &mut Building<'static>,
            &mut Cuts,
            FixedGuardLedgerV1,
            R<()>,
            R<FixedGuardLedgerV1>,
        )>())?,
        // independent borrowed constructor aliases and source shape checks
        frame::<()>(size_of::<(
            FixedGuardInputsV1<'static, 'static>,
            &mut Building<'static>,
            &mut Cuts,
            &SemanticFunctionDeclV1,
            &ProjectedLoopCfgV1,
            &[u8],
            &[Vec<usize>],
            &[bool],
            &[Option<ScalarAssignmentSiteV1>],
            &mut AssertionResourcesV1<'static>,
            usize,
            usize,
            R<AssertionCacheV1<'static>>,
            Option<AssertionCacheV1<'static>>,
        )>())?,
        // independent checked-index original loop
        frame::<()>(size_of::<(
            &SemanticFunctionDeclV1,
            &mut AssertionResourcesV1<'static>,
            &mut Vec<Vec<usize>>,
            &mut Cuts,
            std::iter::Enumerate<
                std::slice::Iter<'static, fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1>,
            >,
            usize,
            &fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1,
            &SemanticTerminatorKindV1,
            &SemanticOperandV1,
            Option<SemanticLocalIdV1>,
            SemanticLocalIdV1,
            Option<&mut Vec<usize>>,
            &mut Vec<usize>,
            R<()>,
            Error,
        )>())?,
        // nonfallible successful assembly transfer
        frame::<PreparedFixedGuardSessionV1<'static>>(size_of::<(
            FixedGuardInputsV1<'static, 'static>,
            FixedGuardLedgerV1,
            Building<'static>,
            SemanticAssertProofsV1<'static>,
            PreparedFixedGuardSessionV1<'static>,
            Option<AssertionCacheV1<'static>>,
        )>())?,
        // owner snapshot branch borrowing
        frame::<Snapshot>(size_of::<(
            &OriginalFixedOracleOwnerV1<'static, 'static, 'static>,
            &Phase<'static, 'static, 'static>,
            &Building<'static>,
            &PreparedFixedGuardSessionV1<'static>,
            RetiredOriginalFixedOracleV1,
            Triple,
            [Triple; 32],
            bool,
            Snapshot,
        )>())?,
        // complete bounded row-roster snapshot construction and iterator
        frame::<(Triple, [Triple; 32], bool)>(size_of::<(
            &Vec<Vec<usize>>,
            [Triple; 32],
            std::iter::Zip<
                std::slice::IterMut<'static, Triple>,
                std::slice::Iter<'static, Vec<usize>>,
            >,
            Option<(&mut Triple, &Vec<usize>)>,
            &mut Triple,
            &Vec<usize>,
            *const usize,
            usize,
            usize,
            *const Vec<usize>,
        )>())?,
        // side owned-arm predicates
        frame::<[bool; 6]>(size_of::<(&Side, [bool; 6])>())?,
        // retired snapshot actual cache map callbacks
        frame::<Snapshot>(size_of::<(
            &RetiredOriginalFixedOracleV1,
            u8,
            Triple,
            [Triple; 32],
            bool,
            Option<&RetiredAssertionCacheV1>,
            Option<(u8, usize, usize, usize)>,
            Snapshot,
            [bool; 6],
        )>())?,
        // ready snapshot all proof/side owned arms
        frame::<Snapshot>(size_of::<(
            &SemanticAssertProofsV1<'static>,
            u8,
            Triple,
            [Triple; 32],
            bool,
            Snapshot,
            [bool; 6],
            Option<(u8, usize, usize, usize)>,
        )>())?,
        // empty side construction
        frame::<Side>(size_of::<Side>())?,
        // empty terminal retirement construction
        frame::<RetiredOriginalFixedOracleV1>(size_of::<(
            RetiredOriginalFixedOracleV1,
            Side,
            Vec<Vec<usize>>,
        )>())?,
        // outer owner retirement dispatch
        frame::<RetiredOriginalFixedOracleV1>(size_of::<(
            OriginalFixedOracleOwnerV1<'static, 'static, 'static>,
            Phase<'static, 'static, 'static>,
            Building<'static>,
            PreparedFixedGuardSessionV1<'static>,
            SemanticAssertProofsV1<'static>,
            RetiredOriginalFixedOracleV1,
        )>())?,
        // Building payload and resource-loan transfer
        frame::<RetiredOriginalFixedOracleV1>(size_of::<(
            AssertionResourcesV1<'static>,
            Vec<Vec<usize>>,
            Option<AssertionCacheV1<'static>>,
            Option<AssertionCacheV1<'static>>,
            RetiredOriginalFixedOracleV1,
            Side,
        )>())?,
        // Ready proof destruction and every owned side payload
        frame::<RetiredOriginalFixedOracleV1>(size_of::<(
            SemanticAssertProofsV1<'static>,
            AssertionGraphV1<'static>,
            AssertionTableV1<'static, u8>,
            AssertionTableV1<'static, Vec<usize>>,
            AssertionTableV1<'static, bool>,
            AssertionTableV1<'static, Option<ScalarAssignmentSiteV1>>,
            Vec<Vec<usize>>,
            Option<StatementDefinitionIndexV1>,
            AssertionCacheV1<'static>,
            AssertionCacheV1<'static>,
            AssertionResourcesV1<'static>,
            RetiredOriginalFixedOracleV1,
            Side,
            Option<ProjectedLoopCfgV1>,
        )>())?,
        // cache Option map transfer, cache primitive separately added twice
        frame::<Option<RetiredAssertionCacheV1>>(size_of::<(
            Option<AssertionCacheV1<'static>>,
            AssertionCacheV1<'static>,
            Option<RetiredAssertionCacheV1>,
            RetiredAssertionCacheV1,
        )>())?,
        // retire u8 table
        frame::<Option<Vec<u8>>>(size_of::<(
            AssertionTableV1<'static, u8>,
            Vec<u8>,
            Option<Vec<u8>>,
        )>())?,
        // retire nested block table
        frame::<Option<Vec<Vec<usize>>>>(size_of::<(
            AssertionTableV1<'static, Vec<usize>>,
            Vec<Vec<usize>>,
            Option<Vec<Vec<usize>>>,
        )>())?,
        // retire escaped bool table
        frame::<Option<Vec<bool>>>(size_of::<(
            AssertionTableV1<'static, bool>,
            Vec<bool>,
            Option<Vec<bool>>,
        )>())?,
        // retire scalar assignment table
        frame::<Option<Vec<Option<ScalarAssignmentSiteV1>>>>(size_of::<(
            AssertionTableV1<'static, Option<ScalarAssignmentSiteV1>>,
            Vec<Option<ScalarAssignmentSiteV1>>,
            Option<Vec<Option<ScalarAssignmentSiteV1>>>,
        )>())?,
        // reserved exclusive empty slot install
        frame::<()>(size_of::<(
            EmptySlot<'static>,
            &mut Option<RetiredOriginalFixedOracleV1>,
            RetiredOriginalFixedOracleV1,
            Option<RetiredOriginalFixedOracleV1>,
        )>())?,
        // actual catch captures and AssertUnwindSafe transfers
        frame::<R<Snapshot>>(size_of::<(
            CatchCaptures,
            AssertUnwindSafe<CatchCaptures>,
            std::thread::Result<R<Snapshot>>,
            R<Snapshot>,
            Panic,
        )>())?,
        // outer match/resume payload same object
        frame::<Snapshot>(size_of::<(
            std::thread::Result<R<Snapshot>>,
            R<Snapshot>,
            Panic,
            &(dyn Any + Send),
            Option<Snapshot>,
        )>())?,
        // original reference entry, session and identical explicit prefix
        frame::<Snapshot>(size_of::<(
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &ProjectedLoopCfgV1,
            &Rich<'static>,
            &mut Prep<'static, 'static>,
            FixedGuardInputsV1<'static, 'static>,
            PreparedFixedGuardSessionV1<'static>,
            Snapshot,
        )>())?,
        // frame composition with checked nested/cache subtotals
        frame::<usize>(size_of::<(
            usize,
            usize,
            usize,
            usize,
            R<usize>,
            Option<usize>,
            Error,
        )>())?,
        // formula row array construction
        frame::<[usize; ADDED_ROWS]>(size_of::<([usize; ADDED_ROWS], Option<usize>)>())?,
        // formula array result and sum call
        frame::<usize>(size_of::<(
            R<[usize; ADDED_ROWS]>,
            [usize; ADDED_ROWS],
            &[usize],
            R<usize>,
        )>())?,
        // formula checked iterator/accumulator
        frame::<usize>(size_of::<(
            &[usize],
            std::slice::Iter<'static, usize>,
            usize,
            &usize,
            Option<usize>,
            R<usize>,
        )>())?,
        // typed frame helper/error construction
        frame::<usize>(size_of::<(
            usize,
            usize,
            usize,
            Option<usize>,
            R<usize>,
            Error,
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1,
        )>())?,
        // snapshot cache borrow/getter return calls
        frame::<(u8, usize, usize, usize)>(size_of::<(
            &AssertionCacheV1<'static>,
            &RetiredAssertionCacheV1,
            Option<&AssertionCacheV1<'static>>,
            Option<&RetiredAssertionCacheV1>,
            Option<(u8, usize, usize, usize)>,
            fn(&AssertionCacheV1<'static>) -> (u8, usize, usize, usize),
            fn(&RetiredAssertionCacheV1) -> (u8, usize, usize, usize),
        )>())?,
        // post-transfer snapshot Option map capture and result
        frame::<Option<Snapshot>>(size_of::<(
            &RetiredOriginalFixedOracleV1,
            &Snapshot,
            Option<&RetiredOriginalFixedOracleV1>,
            Snapshot,
            Option<Snapshot>,
        )>())?,
        // new repeated scalar-count getter in finish_original
        frame::<&[u8]>(size_of::<(&Rich<'static>, &Vec<u8>, &[u8])>())?,
        // new repeated scalar-block getter in finish_original
        frame::<&[Vec<usize>]>(size_of::<(&Rich<'static>, &Vec<Vec<usize>>, &[Vec<usize>])>())?,
        // new repeated address-escaped getter in finish_original
        frame::<&[bool]>(size_of::<(&Rich<'static>, &Vec<bool>, &[bool])>())?,
        // new repeated scalar-assignment getter in finish_original
        frame::<&[Option<ScalarAssignmentSiteV1>]>(size_of::<(
            &Rich<'static>,
            &Vec<Option<ScalarAssignmentSiteV1>>,
            &[Option<ScalarAssignmentSiteV1>],
        )>())?,
    ])
}
pub(in crate::production_ranked_projection_v1) fn added_frame() -> R<usize> {
    let own = added_rows()?.iter().try_fold(0usize, |n, row| {
        n.checked_add(*row)
            .ok_or_else(assertion_resource_overflow_v1)
    })?;
    let nested = nested::additional_frame::<usize>()?;
    let cache =
        AssertionCacheV1::retirement_frame_v1().ok_or_else(assertion_resource_overflow_v1)?;
    own.checked_add(nested)
        .and_then(|n| cache.checked_mul(2).and_then(|c| n.checked_add(c)))
        .ok_or_else(assertion_resource_overflow_v1)
}

#[test]
fn original_constructor_added_frames_are_explicit_and_checked() {
    let rows = added_rows().unwrap();
    assert_eq!(rows.len(), ADDED_ROWS);
    assert_eq!(
        added_frame().unwrap(),
        rows.iter().sum::<usize>()
            + nested::additional_frame::<usize>().unwrap()
            + 2 * AssertionCacheV1::retirement_frame_v1().unwrap()
    );
    assert!(frame::<()>(usize::MAX).is_err());
    assert!(frame::<[u8; 4097]>(0).unwrap() >= 2 * 4097);
}
#[test]
fn original_constructor_empty_retirement_is_lifetime_free_and_allocation_free() {
    fn static_only<T: 'static>() {}
    static_only::<RetiredOriginalFixedOracleV1>();
    let payload = empty_retired();
    let snapshot = payload.snapshot(0);
    assert_eq!((snapshot.checked.1, snapshot.checked.2), (0, 0));
    assert!(snapshot.dominance.is_none() && snapshot.zero.is_none());
    assert_eq!(snapshot.side, [false; 6]);
}

// B1 additive cfg(test) query component; Unit A bodies above stay byte-exact.
#[path = "original_fixed_query_driver_v1.rs"]
mod query_driver;
pub(in crate::production_ranked_projection_v1) use query_driver::{
    QUERY_CAP, QueryCuts, QueryDatum, QuerySource, QueryState, QuerySummary, QueryWitness,
    original_queries, query_added_frame, retained_queries,
};

pub(in crate::production_ranked_projection_v1) use query_driver::{
    CHECKED_ENTRY_CAP, CHECKED_ROW_CAP, CheckedContent, ContentRefusal, ContentWitness,
    ProofContent, content_added_frame, content_added_work, exact_content_matches,
    original_content_queries, retained_content, retained_content_queries,
};
