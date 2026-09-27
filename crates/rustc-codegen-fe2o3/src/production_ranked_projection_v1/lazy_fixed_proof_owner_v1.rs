//! Source-ordered lazy fixed-proof component; no completed producer/factory token.
//! The caller retains this owner and every external Vec until outer postflight.
#![allow(dead_code)]
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_source_preparation_v1::{
    NominalRootSourceTablesV1, RichNominalSourceTablesV1,
};
use fe2o3_mir_model::semantic_mir_v1 as mir;
use std::mem::size_of;
type Error = ProductionRankedProjectionErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Prep<'b, 'w> = PreparationResourcesV1<'b, 'w>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::production_ranked_projection_v1) enum LazyFixedEventV1 {
    Other,
    LiteralSkip,
    NonFixedBoundary,
    Fixed {
        index: SemanticLocalIdV1,
        extent: u64,
    },
}
struct Building<'a> {
    resources: AssertionResourcesV1<'a>,
    checked: Vec<Vec<usize>>,
    dominance: Option<AssertionCacheV1<'a>>,
    zero_exclusion: Option<AssertionCacheV1<'a>>,
}
// Inline variants are covered by owner_frame; boxing would add an unowned allocation.
#[allow(clippy::large_enum_variant)]
enum Phase<'a, 'b, 'w> {
    Pending(Option<&'a mut Prep<'b, 'w>>),
    Building(Building<'a>),
    Ready(PreparedFixedGuardSessionV1<'a>),
    // strict() refused before any proof payload existed, or an internal move
    // has begun. Never an invitation to construct a replacement adapter.
    Unavailable,
}
pub(in crate::production_ranked_projection_v1) struct LazyFixedProofOwnerV1<'a, 'b, 'w> {
    input: FixedGuardInputsV1<'a, 'a>,
    original_ledger: FixedGuardLedgerV1,
    phase: Phase<'a, 'b, 'w>,
    next_block: usize,
    failed: bool,
}
// Source-typed locals for the two reviewed owner vertices. References are
// measured as references; copied source/ledger inputs remain separate fields.
// These are layout expressions only: no tuple is constructed.
type VisitInnerFrameLocalsV1 = (
    &'static mut LazyFixedProofOwnerV1<'static, 'static, 'static>,
    usize,
    &'static SemanticFunctionDeclV1,
    Option<&'static mir::SemanticBasicBlockV1>,
    &'static mir::SemanticBasicBlockV1,
    &'static mir::SemanticTerminatorV1,
    &'static SemanticTerminatorKindV1,
    &'static SemanticOperandV1,
    &'static SemanticOperandV1,
    &'static SemanticOperandV1,
    &'static bool,
    &'static SemanticUnwindActionV1,
    FixedGuardInputsV1<'static, 'static>,
    &'static mut PreparedFixedGuardSessionV1<'static>,
    Result<FixedGuardDataV1>,
    SemanticLocalIdV1,
    u64,
    LazyFixedEventV1,
    Option<usize>,
);
type InitializeBuildingFrameLocalsV1 = (
    FixedGuardInputsV1<'static, 'static>,
    FixedGuardLedgerV1,
    &'static mut Building<'static>,
    &'static mut AssertionResourcesV1<'static>,
    usize,
    usize,
    usize,
    usize,
    usize,
    Option<usize>,
    std::iter::Enumerate<std::slice::Iter<'static, mir::SemanticBasicBlockV1>>,
    Option<(usize, &'static mir::SemanticBasicBlockV1)>,
    &'static mir::SemanticBasicBlockV1,
    &'static mir::SemanticTerminatorV1,
    &'static SemanticTerminatorKindV1,
    &'static SemanticOperandV1,
    Option<SemanticLocalIdV1>,
    SemanticLocalIdV1,
    Option<&'static mut Vec<usize>>,
    &'static mut Vec<usize>,
);
type ConstantOperandFrameLocalsV1 = (
    &'static SemanticOperandV1,
    &'static [Option<u64>],
    ConstantDefinitionV1,
    SemanticLocalIdV1,
    usize,
    Option<&'static Option<u64>>,
    Option<Option<u64>>,
    Option<u64>,
    u64,
);
type ConstantDefinitionFrameLocalsV1 = (
    &'static SemanticOperandV1,
    &'static mir::SemanticConstantV1,
    &'static SemanticConstantValueV1,
    &'static mir::SemanticScalarValueV1,
    &'static SemanticPlaceV1,
    u128,
    std::result::Result<u64, std::num::TryFromIntError>,
    std::result::Result<ConstantDefinitionV1, std::num::TryFromIntError>,
    u64,
    SemanticLocalIdV1,
);
fn accounting() -> Error {
    resource(Resource::Accounting)
}
fn arithmetic() -> Error {
    resource(Resource::Arithmetic)
}
fn frame<R>(locals: usize) -> Result<usize> {
    // Same typed caller/callee return convention as the qualified uniform
    // operand component; no fixed allowance stands in for omitted locals.
    size_of::<R>()
        .checked_mul(2)
        .and_then(|n| n.checked_add(locals))
        .and_then(|n| {
            size_of::<Result<R>>()
                .checked_mul(2)
                .and_then(|r| n.checked_add(r))
        })
        .ok_or_else(arithmetic)
}
fn sum(rows: &[usize]) -> Result<usize> {
    rows.iter()
        .try_fold(0usize, |n, row| n.checked_add(*row).ok_or_else(arithmetic))
}
// Separate source vertices, not optimized stack/RSS measurements. The existing
// strict/evaluator/fixed-guard callees retain their own admissions.
fn owner_frame() -> Result<usize> {
    type O = LazyFixedProofOwnerV1<'static, 'static, 'static>;
    sum(&[
        frame::<O>(size_of::<(
            FixedGuardInputsV1<'static, 'static>,
            &mut Prep<'static, 'static>,
            FixedGuardLedgerV1,
            usize,
        )>())?,
        frame::<O>(size_of::<(
            &NominalRootCfgSourceV1<'static>,
            &mut Prep<'static, 'static>,
        )>())?,
        frame::<()>(size_of::<(&mut O, bool)>())?,
        frame::<()>(size_of::<(
            &mut O,
            usize,
            &mut Prep<'static, 'static>,
            &mut AssertionResourcesV1<'static>,
        )>())?,
        frame::<LazyFixedEventV1>(size_of::<(&mut O, usize, Result<LazyFixedEventV1>)>())?,
        frame::<LazyFixedEventV1>(size_of::<VisitInnerFrameLocalsV1>())?,
        frame::<Option<u64>>(size_of::<ConstantOperandFrameLocalsV1>())?,
        frame::<ConstantDefinitionV1>(size_of::<ConstantDefinitionFrameLocalsV1>())?,
        frame::<()>(size_of::<(
            &mut O,
            Option<&mut Prep<'static, 'static>>,
            Result<AssertionResourcesV1<'static>>,
            Phase<'static, 'static, 'static>,
        )>())?,
        frame::<()>(size_of::<InitializeBuildingFrameLocalsV1>())?,
        frame::<PreparedFixedGuardSessionV1<'static>>(size_of::<(
            Building<'static>,
            SemanticAssertProofsV1<'static>,
            FixedGuardInputsV1<'static, 'static>,
            FixedGuardLedgerV1,
        )>())?,
        frame::<FixedGuardDataV1>(size_of::<(
            &mut PreparedFixedGuardSessionV1<'static>,
            FixedGuardInputsV1<'static, 'static>,
            usize,
        )>())?,
        frame::<Option<SemanticLocalIdV1>>(size_of::<(
            &SemanticOperandV1,
            u32,
            &SemanticPlaceV1,
            &[mir::SemanticProjectionV1],
            &mir::SemanticProjectionV1,
            SemanticProjectionKindV1,
            bool,
            SemanticLocalIdV1,
        )>())?,
        frame::<()>(size_of::<(
            &mut O,
            Option<&mut Prep<'static, 'static>>,
            Result<AssertionResourcesV1<'static>>,
            Building<'static>,
        )>())?,
        frame::<()>(size_of::<(
            &mut Building<'static>,
            Result<AssertionCacheV1<'static>>,
        )>())?,
        frame::<usize>(size_of::<(usize, Option<usize>)>())?,
        // NominalRootCfgSourceV1::source_tables: distinct receiver and caller/callee return transfer.
        frame::<&'static NominalRootSourceTablesV1<'static>>(size_of::<
            &NominalRootCfgSourceV1<'static>,
        >())?,
        // NominalRootCfgSourceV1::function: distinct receiver and caller/callee return transfer.
        frame::<&'static SemanticFunctionDeclV1>(size_of::<&NominalRootCfgSourceV1<'static>>())?,
        // NominalRootCfgSourceV1::graph: distinct receiver and caller/callee return transfer.
        frame::<&'static ProjectedLoopCfgV1>(size_of::<&NominalRootCfgSourceV1<'static>>())?,
        // NominalRootCfgSourceV1::types: distinct receiver and caller/callee return transfer.
        frame::<&'static [SemanticTypeDeclV1]>(size_of::<&NominalRootCfgSourceV1<'static>>())?,
        // NominalRootSourceTablesV1::rich: distinct receiver and caller/callee return transfer.
        frame::<&'static RichNominalSourceTablesV1<'static>>(size_of::<
            &NominalRootSourceTablesV1<'static>,
        >())?,
        // RichNominalSourceTablesV1::function: distinct receiver and caller/callee return transfer.
        frame::<&'static SemanticFunctionDeclV1>(size_of::<&RichNominalSourceTablesV1<'static>>())?,
        // RichNominalSourceTablesV1::belongs_to_original_ledger_v1: distinct receiver and caller/callee return transfer.
        frame::<bool>(size_of::<(
            &RichNominalSourceTablesV1<'static>,
            FixedGuardLedgerV1,
        )>())?,
        // RichNominalSourceTablesV1::constants: distinct receiver and caller/callee return transfer.
        frame::<&'static [Option<u64>]>(size_of::<&RichNominalSourceTablesV1<'static>>())?,
        // RichNominalSourceTablesV1::scalar_counts: distinct receiver and caller/callee return transfer.
        frame::<&'static [u8]>(size_of::<&RichNominalSourceTablesV1<'static>>())?,
        // RichNominalSourceTablesV1::scalar_blocks: distinct receiver and caller/callee return transfer.
        frame::<&'static [Vec<usize>]>(size_of::<&RichNominalSourceTablesV1<'static>>())?,
        // RichNominalSourceTablesV1::scalar_assignments: distinct receiver and caller/callee return transfer.
        frame::<&'static [Option<ScalarAssignmentSiteV1>]>(size_of::<
            &RichNominalSourceTablesV1<'static>,
        >())?,
        // RichNominalSourceTablesV1::address_escaped: distinct receiver and caller/callee return transfer.
        frame::<&'static [bool]>(size_of::<&RichNominalSourceTablesV1<'static>>())?,
        // SemanticFunctionDeclV1::locals: distinct receiver and caller/callee return transfer.
        frame::<&'static [mir::SemanticLocalDeclV1]>(size_of::<&SemanticFunctionDeclV1>())?,
        // SemanticFunctionDeclV1::blocks: distinct receiver and caller/callee return transfer.
        frame::<&'static [mir::SemanticBasicBlockV1]>(size_of::<&SemanticFunctionDeclV1>())?,
        // SemanticFunctionDeclV1::entry: distinct receiver and caller/callee return transfer.
        frame::<SemanticBlockIdV1>(size_of::<&SemanticFunctionDeclV1>())?,
        // SemanticBasicBlockV1::terminator: distinct receiver and caller/callee return transfer.
        frame::<&'static mir::SemanticTerminatorV1>(size_of::<&mir::SemanticBasicBlockV1>())?,
        // SemanticTerminatorV1::kind: distinct receiver and caller/callee return transfer.
        frame::<&'static SemanticTerminatorKindV1>(size_of::<&mir::SemanticTerminatorV1>())?,
        // SemanticConstantV1::value: distinct receiver and caller/callee return transfer.
        frame::<&'static SemanticConstantValueV1>(size_of::<&mir::SemanticConstantV1>())?,
        // SemanticScalarValueV1::bits: distinct receiver and caller/callee return transfer.
        frame::<u128>(size_of::<mir::SemanticScalarValueV1>())?,
        // SemanticPlaceV1::projections: distinct receiver and caller/callee return transfer.
        frame::<&'static [mir::SemanticProjectionV1]>(size_of::<&SemanticPlaceV1>())?,
        // SemanticPlaceV1::local: distinct receiver and caller/callee return transfer.
        frame::<SemanticLocalIdV1>(size_of::<&SemanticPlaceV1>())?,
        // SemanticProjectionV1::kind: distinct receiver and caller/callee return transfer.
        frame::<SemanticProjectionKindV1>(size_of::<mir::SemanticProjectionV1>())?,
        // SemanticLocalIdV1::index: distinct receiver and caller/callee return transfer.
        frame::<u32>(size_of::<SemanticLocalIdV1>())?,
        // SemanticBlockIdV1::index: distinct receiver and caller/callee return transfer.
        frame::<u32>(size_of::<SemanticBlockIdV1>())?,
        frame::<usize>(size_of::<[usize; 44]>())?,
        frame::<usize>(size_of::<(&[usize], usize, &usize, Option<usize>)>())?,
        frame::<Error>(size_of::<Resource>())?,
        frame::<Error>(size_of::<Resource>())?,
    ])
}
// One concrete generic call pays both alternatives conservatively. Required
// assertion wrapper, generic push/reserve, four sealed trait methods and
// both mapper closures are distinct; original primitive callee contracts stay.
fn facet_frame<T>() -> Result<usize> {
    type A = AssertionResourcesV1<'static>;
    type P = Prep<'static, 'static>;
    sum(&[
        frame::<()>(size_of::<(
            &mut LazyFixedProofOwnerV1<'static, 'static, 'static>,
            &mut Vec<T>,
            usize,
            Result<()>,
        )>())?,
        frame::<()>(size_of::<(
            &mut LazyFixedProofOwnerV1<'static, 'static, 'static>,
            &mut Vec<T>,
            &mut Option<T>,
            Option<T>,
            T,
            Result<()>,
        )>())?,
        frame::<()>(size_of::<(
            &mut LazyFixedProofOwnerV1<'static, 'static, 'static>,
            usize,
            Result<usize>,
        )>())?,
        frame::<()>(size_of::<(&mut A, &mut Vec<T>, usize, Result<()>)>())?,
        frame::<()>(size_of::<(&mut A, &mut Vec<T>, T, Result<()>)>())?,
        frame::<()>(size_of::<(
            &mut A,
            &mut Vec<T>,
            usize,
            usize,
            Option<usize>,
            bool,
            std::result::Result<(), std::collections::TryReserveError>,
        )>())?,
        frame::<()>(size_of::<(&mut A, &mut Vec<T>, T, usize)>())?,
        frame::<()>(size_of::<(&A, Result<()>)>())?,
        frame::<bool>(size_of::<&A>())?,
        frame::<()>(size_of::<(&mut A, usize, Result<()>)>())?,
        frame::<()>(size_of::<(&mut A, usize, Result<()>)>())?,
        frame::<Error>(size_of::<(&mut A, Error)>())?,
        frame::<Error>(size_of::<(&mut A, Error)>())?,
        frame::<Error>(size_of::<std::collections::TryReserveError>())?,
        frame::<Error>(size_of::<Option<usize>>())?,
        frame::<()>(size_of::<(
            &mut P,
            &mut Vec<T>,
            usize,
            usize,
            Option<usize>,
            bool,
            std::result::Result<(), std::collections::TryReserveError>,
        )>())?,
        frame::<()>(size_of::<(&mut P, &mut Vec<T>, T)>())?,
        frame::<usize>(size_of::<(usize, Option<usize>)>())?,
        frame::<usize>(size_of::<[usize; 20]>())?,
        frame::<usize>(size_of::<(&[usize], usize, &usize, Option<usize>)>())?,
    ])
}
impl<'a, 'b, 'w> LazyFixedProofOwnerV1<'a, 'b, 'w> {
    fn new(input: FixedGuardInputsV1<'a, 'a>, resources: &'a mut Prep<'b, 'w>) -> Result<Self> {
        if !resources.is_metered() || resources.has_denial() {
            return Err(accounting());
        }
        let ledger = resources.original_ledger_v1().ok_or_else(accounting)?;
        if !std::ptr::eq(input.function, input.rich.function())
            || !input.rich.belongs_to_original_ledger_v1(ledger)
            || input.graph.entry != input.function.entry().index() as usize
        {
            return Err(Error::Unsupported(
                "lazy fixed proof differs from its original source loan",
            ));
        }
        let bytes = owner_frame()?;
        resources.work(bytes)?;
        resources.reserve_storage(bytes)?;
        Ok(Self {
            input,
            original_ledger: ledger,
            phase: Phase::Pending(Some(resources)),
            next_block: 0,
            failed: false,
        })
    }
    fn enter(&mut self) -> Result<()> {
        if self.failed {
            return Err(accounting());
        }
        // Remains true after every Err or unwinding call. No Drop-based retry.
        self.failed = true;
        Ok(())
    }
    fn tick(&mut self, work: usize) -> Result<()> {
        match &mut self.phase {
            Phase::Pending(Some(resources)) => {
                if resources.has_denial()
                    || resources.original_ledger_v1() != Some(self.original_ledger)
                {
                    return Err(accounting());
                }
                resources.work(work)
            }
            Phase::Ready(session) => session.proof.resources.extra_work(work),
            _ => Err(accounting()),
        }
    }
    fn pay_facet<T>(&mut self) -> Result<()> {
        let bytes = facet_frame::<T>()?;
        self.tick(bytes)?;
        match &mut self.phase {
            Phase::Pending(Some(resources)) => resources.reserve_storage(bytes),
            Phase::Ready(session) => session.proof.resources.reserve_storage(bytes),
            _ => Err(accounting()),
        }
    }
    /// Vec and payload are caller-retained DATA, not a producer token.
    pub(in crate::production_ranked_projection_v1) fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
    ) -> Result<()> {
        self.enter()?;
        self.pay_facet::<T>()?;
        match &mut self.phase {
            Phase::Pending(Some(resources)) => resources.reserve(values, additional)?,
            Phase::Ready(session) => session
                .proof
                .resources
                .preparation_reserve_v1(values, additional)?,
            _ => return Err(accounting()),
        }
        self.failed = false;
        Ok(())
    }
    /// A refused push keeps the candidate physically in the caller's slot.
    /// Take happens only after original work(1) and exact reserve both succeed.
    pub(in crate::production_ranked_projection_v1) fn push_retained<T>(
        &mut self,
        values: &mut Vec<T>,
        candidate: &mut Option<T>,
    ) -> Result<()> {
        self.enter()?;
        if candidate.is_none() {
            return Err(accounting());
        }
        self.pay_facet::<T>()?;
        self.tick(1)?;
        match &mut self.phase {
            Phase::Pending(Some(resources)) => resources.reserve(values, 1)?,
            Phase::Ready(session) => session.proof.resources.preparation_reserve_v1(values, 1)?,
            _ => return Err(accounting()),
        }
        values.push(
            candidate
                .take()
                .expect("retained candidate checked before fallible reserve"),
        );
        self.failed = false;
        Ok(())
    }
    /// Exactly one next source block. NonFixedBoundary is only a scheduling
    /// boundary; this component does not execute/authorize its slice stage.
    pub(in crate::production_ranked_projection_v1) fn visit(
        &mut self,
        block: usize,
    ) -> Result<LazyFixedEventV1> {
        self.enter()?;
        let result = self.visit_inner(block);
        if result.is_ok() {
            self.failed = false;
        }
        result
    }
    fn visit_inner(&mut self, block: usize) -> Result<LazyFixedEventV1> {
        self.tick(1)?;
        if block != self.next_block {
            return Err(Error::Unsupported(
                "lazy fixed proof source event is out of order",
            ));
        }
        let function = self.input.function;
        let source = function.blocks().get(block).ok_or(Error::Unsupported(
            "lazy fixed proof source event is outside its original function",
        ))?;
        let event = match source.terminator().kind() {
            SemanticTerminatorKindV1::Assert {
                condition: _,
                expected,
                message: SemanticAssertMessageV1::BoundsCheck { length, index },
                target: _,
                unwind,
            } => {
                if !*expected || !matches!(unwind, SemanticUnwindActionV1::Unreachable) {
                    return Err(Error::Incomplete(
                        "a Rust bounds check without the canonical success/unreachable shape",
                    ));
                }
                // Exact original left-to-right constant test. Neither helper
                // traverses projections: constant_definition checks emptiness.
                self.tick(1)?;
                if constant_operand_value(length, self.input.rich.constants()).is_some() && {
                    self.tick(1)?;
                    constant_operand_value(index, self.input.rich.constants()).is_some()
                } {
                    LazyFixedEventV1::LiteralSkip
                } else if matches!(length, SemanticOperandV1::Constant(_)) {
                    self.activate()?;
                    let input = self.input;
                    let Phase::Ready(session) = &mut self.phase else {
                        return Err(accounting());
                    };
                    let (index, extent) = session.query_inputs(input, block)?;
                    LazyFixedEventV1::Fixed { index, extent }
                } else {
                    LazyFixedEventV1::NonFixedBoundary
                }
            }
            SemanticTerminatorKindV1::SwitchInt { .. } => LazyFixedEventV1::NonFixedBoundary,
            _ => LazyFixedEventV1::Other,
        };
        self.next_block = self.next_block.checked_add(1).ok_or_else(arithmetic)?;
        Ok(event)
    }
    fn activate(&mut self) -> Result<()> {
        if matches!(self.phase, Phase::Ready(_)) {
            return Ok(());
        }
        self.install_strict()?;
        let Phase::Building(building) = &mut self.phase else {
            return Err(accounting());
        };
        initialize_building(self.input, self.original_ledger, building)?;
        initialize_caches(building)?;
        // No fallible/allocation/user-code operation after taking the payload.
        // Every Option was installed by initialize_building's successful end.
        let Phase::Building(building) = std::mem::replace(&mut self.phase, Phase::Unavailable)
        else {
            unreachable!()
        };
        self.phase = Phase::Ready(finish_building(self.input, self.original_ledger, building));
        Ok(())
    }
    fn install_strict(&mut self) -> Result<()> {
        let resources = match &mut self.phase {
            Phase::Pending(slot) => slot.take().ok_or_else(accounting)?,
            _ => return Err(accounting()),
        };
        self.phase = Phase::Unavailable;
        // If strict refuses, it has allocated no proof/Vec/cache. Its accepted
        // credits stay with the original outer owner; this owner stays terminal.
        let strict = AssertionResourcesV1::strict(resources)?;
        self.phase = Phase::Building(Building {
            resources: strict,
            checked: Vec::new(),
            dominance: None,
            zero_exclusion: None,
        });
        Ok(())
    }
}
fn initialize_building(
    input: FixedGuardInputsV1<'_, '_>,
    ledger: FixedGuardLedgerV1,
    building: &mut Building<'_>,
) -> Result<()> {
    let resources = &mut building.resources;
    reserve_fixed_guard_frames_v1(resources)?;
    resources.extra_work(1)?;
    if !std::ptr::eq(input.function, input.rich.function())
        || !input.rich.belongs_to_original_ledger_v1(ledger)
    {
        return Err(Error::Unsupported(
            "prepared fixed guard differs from its original source loan",
        ));
    }
    reserve_assertion_evaluator_frames_v1(resources)?;
    resources.reserve_frame::<SemanticAssertProofsV1<'_>>(0)?;
    resources.extra_work(64)?;
    let locals = input.function.locals().len();
    let blocks = input.function.blocks().len();
    if !resources.is_strict()
        || input.rich.scalar_counts().len() != locals
        || input.rich.address_escaped().len() != locals
        || input.rich.scalar_assignments().len() != locals
        || input.rich.scalar_blocks().len() != blocks
        || input.graph.successors.len() != blocks
        || input.graph.predecessors.len() != blocks
        || input.graph.reachable.len() != blocks
        || input.graph.entry != input.function.entry().index() as usize
    {
        return Err(Error::Unsupported(
            "prepared assertion source tables differ from the exact function",
        ));
    }
    // Original strict nested-table policy, but reserve/populate the retained
    // owner field rather than a fallible constructor's local return Vec.
    let work = locals
        .checked_mul(size_of::<Vec<usize>>())
        .and_then(|n| n.checked_add(locals))
        .ok_or_else(arithmetic)?;
    resources.extra_work(work)?;
    resources.reserve_vec(
        &mut building.checked,
        locals,
        assertion_resources_v1::LegacyReserve::Exact,
        "assertion nested table storage cannot be reserved",
    )?;
    building.checked.resize_with(locals, Vec::new);
    resources.extra_work(blocks)?;
    for (block_index, block) in input.function.blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Assert { condition, .. } = block.terminator().kind() else {
            continue;
        };
        let Some(local) = tuple_field_operand_local_v1(condition, 1) else {
            continue;
        };
        let Some(rows) = building.checked.get_mut(local.index() as usize) else {
            return Err(Error::Unsupported(
                "a checked arithmetic assertion is outside the semantic local table",
            ));
        };
        resources.push_vec(
            rows,
            block_index,
            "checked arithmetic assertion storage cannot be reserved",
        )?;
    }
    Ok(())
}
fn initialize_caches(building: &mut Building<'_>) -> Result<()> {
    building.dominance = Some(AssertionCacheV1::new(&mut building.resources)?);
    building.zero_exclusion = Some(AssertionCacheV1::new(&mut building.resources)?);
    Ok(())
}
fn finish_building<'a>(
    input: FixedGuardInputsV1<'a, 'a>,
    original_ledger: FixedGuardLedgerV1,
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
            .expect("successful lazy initialization installs dominance"),
        zero_exclusion: building
            .zero_exclusion
            .expect("successful lazy initialization installs zero exclusion"),
        work: 0,
        resources: building.resources,
    };
    PreparedFixedGuardSessionV1 {
        proof,
        original_ledger,
        failed: false,
    }
}
pub(in crate::production_ranked_projection_v1) fn prepare_lazy_fixed_proof_owner_v1<'a, 'b, 'w>(
    view: &'a NominalRootCfgSourceV1<'_>,
    resources: &'a mut Prep<'b, 'w>,
) -> Result<LazyFixedProofOwnerV1<'a, 'b, 'w>> {
    LazyFixedProofOwnerV1::new(
        FixedGuardInputsV1 {
            types: view.types(),
            function: view.function(),
            graph: view.graph(),
            rich: view.source_tables().rich(),
        },
        resources,
    )
}
#[cfg(test)]
impl<'a, 'b, 'w> LazyFixedProofOwnerV1<'a, 'b, 'w> {
    pub(in crate::production_ranked_projection_v1) fn for_test(
        types: &'a [SemanticTypeDeclV1],
        function: &'a SemanticFunctionDeclV1,
        graph: &'a ProjectedLoopCfgV1,
        rich: &'a RichNominalSourceTablesV1<'a>,
        resources: &'a mut Prep<'b, 'w>,
    ) -> Result<Self> {
        // Separate concrete component caller frame, no borrowed-table authority.
        if !resources.is_metered() || resources.has_denial() {
            return Err(accounting());
        }
        let bytes = sum(&[
            frame::<Self>(size_of::<(FixedGuardInputsV1<'a, 'a>, &mut Prep<'b, 'w>)>())?,
            frame::<(u8, usize, bool, usize, usize, usize)>(
                size_of::<(&Self, &Phase<'a, 'b, 'w>)>(),
            )?,
            frame::<()>(size_of::<(&mut Self, &mut Prep<'b, 'w>, Result<()>)>())?,
            frame::<()>(size_of::<(
                &mut Self,
                &mut Prep<'b, 'w>,
                &mut AssertionResourcesV1<'a>,
                Result<()>,
            )>())?,
            frame::<()>(size_of::<(&mut Self, &mut Building<'a>, Result<()>)>())?,
            frame::<()>(size_of::<(&mut Self, &mut Building<'a>, Result<()>)>())?,
        ])?;
        resources.work(bytes)?;
        resources.reserve_storage(bytes)?;
        Self::new(
            FixedGuardInputsV1 {
                types,
                function,
                graph,
                rich,
            },
            resources,
        )
    }
    pub(in crate::production_ranked_projection_v1) fn deny_available_for_test(&mut self) {
        match &mut self.phase {
            Phase::Pending(Some(resources)) => {
                let _ = resources.work(usize::MAX);
            }
            Phase::Ready(session) => {
                let _ = session.proof.resources.extra_work(usize::MAX);
            }
            _ => panic!("component expected usable phase"),
        }
    }
    pub(in crate::production_ranked_projection_v1) fn state_for_test(
        &self,
    ) -> (u8, usize, bool, usize, usize, usize) {
        match &self.phase {
            Phase::Pending(_) => (0, self.next_block, self.failed, 0, 0, 0),
            Phase::Building(b) => (
                1,
                self.next_block,
                self.failed,
                b.checked.len(),
                b.checked.capacity(),
                usize::from(b.dominance.is_some()) + usize::from(b.zero_exclusion.is_some()),
            ),
            Phase::Ready(s) => (
                2,
                self.next_block,
                self.failed,
                s.proof.checked_assertion_blocks.len(),
                s.proof.checked_assertion_blocks.capacity(),
                s.proof.work,
            ),
            Phase::Unavailable => (3, self.next_block, self.failed, 0, 0, 0),
        }
    }
    pub(in crate::production_ranked_projection_v1) fn denial_before_strict_for_test(
        &mut self,
    ) -> Result<()> {
        self.enter()?;
        let Phase::Pending(Some(resources)) = &mut self.phase else {
            return Err(accounting());
        };
        let _ = resources.work(usize::MAX);
        self.install_strict()
    }
    pub(in crate::production_ranked_projection_v1) fn deny_after_index_for_test(
        &mut self,
    ) -> Result<()> {
        self.enter()?;
        self.install_strict()?;
        let Phase::Building(building) = &mut self.phase else {
            return Err(accounting());
        };
        initialize_building(self.input, self.original_ledger, building)?;
        let _ = building.resources.extra_work(usize::MAX);
        initialize_caches(building)
    }
    pub(in crate::production_ranked_projection_v1) fn panic_after_initialization_for_test(
        &mut self,
    ) {
        self.enter().expect("fresh component");
        self.install_strict().expect("strict component");
        let Phase::Building(building) = &mut self.phase else {
            unreachable!()
        };
        initialize_building(self.input, self.original_ledger, building)
            .expect("initialized component");
        initialize_caches(building).expect("initialized caches");
        panic!("intentional retained Building unwind");
    }
}

#[cfg(test)]
#[path = "lazy_fixed_proof_frame_v1_tests.rs"]
mod frame_controls;
