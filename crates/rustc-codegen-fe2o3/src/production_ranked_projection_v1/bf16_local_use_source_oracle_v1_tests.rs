//! Independent cfg(test) source/local decision oracle. No production admission.
//! This leaf never calls the production selector, scalar inventory or local query.
use super::bf16_nominal_preparation_resources_v1::{PreparationResourcesV1, resource};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
type OracleResult<T> = std::result::Result<T, ProductionRankedProjectionErrorV1>;

#[derive(Default)]
pub(super) struct ScalarOracle {
    pub(super) counts: Vec<u8>,
    pub(super) assignments: Vec<Option<ScalarAssignmentSiteV1>>,
    pub(super) escaped: Vec<bool>,
    started: bool,
}
impl ScalarOracle {
    pub(super) fn prepare(
        &mut self,
        f: &SemanticFunctionDeclV1,
        r: &mut PreparationResourcesV1<'_, '_>,
    ) -> OracleResult<()> {
        if self.started || r.has_denial() || r.original_ledger_v1().is_none() {
            return Err(resource(Resource::Accounting));
        }
        r.work(64)?;
        self.started = true;
        let n = f.locals().len();
        r.work(
            n.checked_mul(4)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )?;
        r.reserve(&mut self.counts, n)?;
        self.counts.resize(n, 0);
        r.reserve(&mut self.assignments, n)?;
        self.assignments.resize(n, None);
        r.reserve(&mut self.escaped, n)?;
        self.escaped.resize(n, false);
        for (bi, block) in f.blocks().iter().enumerate() {
            r.work(32)?;
            for (si, statement) in block.statements().iter().enumerate() {
                r.work(128)?;
                if let SemanticStatementKindV1::Assign(a) = statement.kind() {
                    if a.destination().projections().is_empty() {
                        let local = a.destination().local().index() as usize;
                        let slot = self.assignments.get_mut(local).ok_or(
                            ProductionRankedProjectionErrorV1::Incomplete(
                                "local oracle assignment coordinate absent",
                            ),
                        )?;
                        if slot.is_none() {
                            *slot = Some(ScalarAssignmentSiteV1 {
                                block: bi,
                                statement: si,
                            });
                        }
                    }
                    let escaped_place = match a.value().kind() {
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Mutable,
                            place,
                        }
                        | SemanticRvalueKindV1::AddressOf { place, .. } => Some(place),
                        _ => None,
                    };
                    if let Some(p) = escaped_place
                        && !matches!(
                            p.projections().first().map(|p| p.kind()),
                            Some(SemanticProjectionKindV1::Dereference)
                        )
                    {
                        let slot = self.escaped.get_mut(p.local().index() as usize).ok_or(
                            ProductionRankedProjectionErrorV1::Incomplete(
                                "local oracle escape coordinate absent",
                            ),
                        )?;
                        *slot = true;
                    }
                }
                // Explicit independent definition cases; no shared visitor.
                match statement.kind() {
                    SemanticStatementKindV1::Assign(a) => self.definition(a.destination())?,
                    SemanticStatementKindV1::Store(a) => self.definition(a.destination())?,
                    SemanticStatementKindV1::AtomicRmw(a) => {
                        self.definition(a.destination())?;
                        self.definition(a.address())?;
                    }
                    SemanticStatementKindV1::AtomicCompareExchange(a) => {
                        self.definition(a.destination())?;
                        self.definition(a.address())?;
                    }
                    SemanticStatementKindV1::SetDiscriminant { place, .. }
                    | SemanticStatementKindV1::Deinitialize(place) => self.definition(place)?,
                    SemanticStatementKindV1::StorageLive(_)
                    | SemanticStatementKindV1::StorageDead(_)
                    | SemanticStatementKindV1::Assume(_)
                    | SemanticStatementKindV1::Nop => {}
                }
            }
            if let SemanticTerminatorKindV1::Call(c) = block.terminator().kind() {
                r.work(32)?;
                if let Some(d) = c.destination() {
                    self.definition(d.place())?;
                }
            }
        }
        for (i, a) in self.assignments.iter_mut().enumerate() {
            r.work(1)?;
            if self.counts[i] != 1 {
                *a = None;
            }
        }
        Ok(())
    }
    fn definition(&mut self, p: &SemanticPlaceV1) -> OracleResult<()> {
        if matches!(
            p.projections().first().map(|p| p.kind()),
            Some(SemanticProjectionKindV1::Dereference)
        ) {
            return Ok(());
        }
        let slot = self.counts.get_mut(p.local().index() as usize).ok_or(
            ProductionRankedProjectionErrorV1::Incomplete(
                "local oracle definition coordinate absent",
            ),
        )?;
        *slot = slot.saturating_add(1);
        Ok(())
    }
    pub(super) fn immutable(&self, f: &SemanticFunctionDeclV1, local: usize) -> OracleResult<bool> {
        let declaration =
            f.locals()
                .get(local)
                .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                    "local oracle query coordinate absent",
                ))?;
        if self.counts.len() != f.locals().len()
            || self.assignments.len() != f.locals().len()
            || self.escaped.len() != f.locals().len()
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "local oracle unfinished rows",
            ));
        }
        Ok(!declaration.role().is_entry_argument()
            && self.counts[local] == 1
            && self.assignments[local].is_some()
            && !self.escaped[local])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Coordinate {
    pub(super) block: usize,
    pub(super) statement: Option<usize>,
    pub(super) ordinal: usize,
}
pub(super) struct Occurrence<'a> {
    pub(super) coordinate: Coordinate,
    pub(super) place: &'a SemanticPlaceV1,
    pub(super) access: AccessKindAttr,
    pub(super) atomic: Option<SemanticAtomicAccessV1>,
    pub(super) requirement: PlaceAccessRequirementV1,
    pub(super) source: SemanticSourceProvenanceV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OriginDecision {
    Allowed(Option<CheckedReferenceSourceV1>),
    RegionRefused,
}

/// Retained model dominance facts are shared INPUTS, not re-proved by this oracle.
pub(super) fn expected_origin(
    p: &SemanticPlaceV1,
    block: usize,
    origins: &[Option<CheckedReferenceOriginV1>],
    option: &SemanticOptionDominanceV1,
    enums: &SemanticEnumPayloadDominanceV1,
    r: &mut PreparationResourcesV1<'_, '_>,
) -> OracleResult<OriginDecision> {
    r.work(
        p.projections()
            .len()
            .checked_add(32)
            .ok_or_else(|| resource(Resource::Arithmetic))?,
    )?;
    let Some(origin) = origins.get(p.local().index() as usize).copied().flatten() else {
        return Ok(OriginDecision::Allowed(None));
    };
    let mut projections = p.projections().iter();
    if !matches!(
        projections.next().map(|p| p.kind()),
        Some(SemanticProjectionKindV1::Dereference)
    ) {
        return Ok(OriginDecision::Allowed(None));
    }
    for projection in projections {
        if !matches!(
            projection.kind(),
            SemanticProjectionKindV1::Field(_)
                | SemanticProjectionKindV1::Downcast(_)
                | SemanticProjectionKindV1::OpaqueCast
                | SemanticProjectionKindV1::Subtype
        ) {
            return Ok(OriginDecision::Allowed(None));
        }
    }
    let block = u32::try_from(block).map_err(|_| resource(Resource::Arithmetic))?;
    let allowed = match origin.availability {
        None => true,
        Some(CapabilityAvailabilityV1::Option(a)) => {
            option.allows(a, SemanticBlockIdV1::from_index(block))
        }
        Some(CapabilityAvailabilityV1::EnumPayload(a)) => {
            enums.allows(a, SemanticBlockIdV1::from_index(block))
        }
    };
    Ok(if allowed {
        OriginDecision::Allowed(Some(origin.source))
    } else {
        OriginDecision::RegionRefused
    })
}
pub(super) fn eligible_some(origin: CheckedReferenceSourceV1, o: &Occurrence<'_>) -> bool {
    if o.access.is_atomic() != o.atomic.is_some() {
        return false;
    }
    match origin {
        CheckedReferenceSourceV1::GuardedAccess(_) => o.atomic.is_none(),
        CheckedReferenceSourceV1::ProjectedSharedBorrow => {
            o.atomic.is_none() && o.access == AccessKindAttr::Read
        }
    }
}

struct Walk<'a, 'v, F> {
    block: usize,
    statement: Option<usize>,
    ordinal: usize,
    source: SemanticSourceProvenanceV1,
    visit: &'v mut F,
    marker: std::marker::PhantomData<&'a SemanticFunctionDeclV1>,
}
impl<'a, F> Walk<'a, '_, F>
where
    F: FnMut(Occurrence<'a>, &mut PreparationResourcesV1<'_, '_>) -> OracleResult<()>,
{
    fn place(
        &mut self,
        p: &'a SemanticPlaceV1,
        access: AccessKindAttr,
        atomic: Option<SemanticAtomicAccessV1>,
        requirement: PlaceAccessRequirementV1,
        r: &mut PreparationResourcesV1<'_, '_>,
    ) -> OracleResult<()> {
        r.work(64)?;
        let ordinal = self.ordinal;
        self.ordinal = self
            .ordinal
            .checked_add(1)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        (self.visit)(
            Occurrence {
                coordinate: Coordinate {
                    block: self.block,
                    statement: self.statement,
                    ordinal,
                },
                place: p,
                access,
                atomic,
                requirement,
                source: self.source,
            },
            r,
        )
    }
    fn operand(
        &mut self,
        operand: &'a SemanticOperandV1,
        r: &mut PreparationResourcesV1<'_, '_>,
    ) -> OracleResult<()> {
        r.work(16)?;
        match operand {
            SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p) => self.place(
                p,
                AccessKindAttr::Read,
                None,
                PlaceAccessRequirementV1::IfMemory,
                r,
            ),
            SemanticOperandV1::Constant(_) => Ok(()),
        }
    }
    fn rvalue(
        &mut self,
        value: &'a SemanticRvalueKindV1,
        r: &mut PreparationResourcesV1<'_, '_>,
    ) -> OracleResult<()> {
        r.work(32)?;
        match value {
            SemanticRvalueKindV1::Use(v)
            | SemanticRvalueKindV1::Unary { operand: v, .. }
            | SemanticRvalueKindV1::Cast { operand: v, .. } => self.operand(v, r),
            SemanticRvalueKindV1::Binary { left, right, .. } => {
                self.operand(left, r)?;
                self.operand(right, r)
            }
            SemanticRvalueKindV1::CheckedBinary(v) => {
                self.operand(v.left(), r)?;
                self.operand(v.right(), r)
            }
            SemanticRvalueKindV1::UncheckedBinary(v) => {
                self.operand(v.left(), r)?;
                self.operand(v.right(), r)
            }
            SemanticRvalueKindV1::Aggregate(v) => {
                for operand in v.operands() {
                    self.operand(operand, r)?;
                }
                Ok(())
            }
            SemanticRvalueKindV1::Load(v) => self.place(
                v.source(),
                if v.atomic().is_some() {
                    AccessKindAttr::AtomicRead
                } else {
                    AccessKindAttr::Read
                },
                v.atomic(),
                PlaceAccessRequirementV1::ExplicitMemory,
                r,
            ),
            SemanticRvalueKindV1::Length(p) | SemanticRvalueKindV1::Discriminant(p) => self.place(
                p,
                AccessKindAttr::Read,
                None,
                PlaceAccessRequirementV1::IfMemory,
                r,
            ),
            SemanticRvalueKindV1::Borrow { .. } | SemanticRvalueKindV1::AddressOf { .. } => Ok(()),
        }
    }
}

/// Streaming independent scan. The callback may read/query, never derives the
/// occurrence by invoking the production selector. Call/drop sites stay separate.
pub(super) fn walk_occurrences<'a, F, G>(
    f: &'a SemanticFunctionDeclV1,
    r: &mut PreparationResourcesV1<'_, '_>,
    visit: &mut F,
    refused_call: &mut G,
) -> OracleResult<()>
where
    F: FnMut(Occurrence<'a>, &mut PreparationResourcesV1<'_, '_>) -> OracleResult<()>,
    G: FnMut(Coordinate, &mut PreparationResourcesV1<'_, '_>) -> OracleResult<()>,
{
    if r.has_denial() || r.original_ledger_v1().is_none() {
        return Err(resource(Resource::Accounting));
    }
    for (bi, block) in f.blocks().iter().enumerate() {
        r.work(64)?;
        for (si, statement) in block.statements().iter().enumerate() {
            r.work(64)?;
            let mut w = Walk {
                block: bi,
                statement: Some(si),
                ordinal: 0,
                source: statement.source(),
                visit: &mut *visit,
                marker: std::marker::PhantomData,
            };
            match statement.kind() {
                SemanticStatementKindV1::Assign(v) => {
                    w.rvalue(v.value().kind(), r)?;
                    w.place(
                        v.destination(),
                        AccessKindAttr::Write,
                        None,
                        PlaceAccessRequirementV1::IfMemory,
                        r,
                    )?;
                }
                SemanticStatementKindV1::Store(v) => {
                    w.operand(v.value(), r)?;
                    w.place(
                        v.destination(),
                        if v.atomic().is_some() {
                            AccessKindAttr::AtomicWrite
                        } else {
                            AccessKindAttr::Write
                        },
                        v.atomic(),
                        PlaceAccessRequirementV1::ExplicitMemory,
                        r,
                    )?;
                }
                SemanticStatementKindV1::AtomicRmw(v) => {
                    w.operand(v.value(), r)?;
                    w.place(
                        v.address(),
                        AccessKindAttr::AtomicReadModifyWrite,
                        Some(v.access()),
                        PlaceAccessRequirementV1::ExplicitMemory,
                        r,
                    )?;
                    w.place(
                        v.destination(),
                        AccessKindAttr::Write,
                        None,
                        PlaceAccessRequirementV1::IfMemory,
                        r,
                    )?;
                }
                SemanticStatementKindV1::AtomicCompareExchange(v) => {
                    w.place(
                        v.destination(),
                        AccessKindAttr::Write,
                        None,
                        PlaceAccessRequirementV1::IfMemory,
                        r,
                    )?;
                    w.place(
                        v.address(),
                        AccessKindAttr::AtomicReadModifyWrite,
                        Some(v.success()),
                        PlaceAccessRequirementV1::ExplicitMemory,
                        r,
                    )?;
                    w.operand(v.expected(), r)?;
                    w.operand(v.replacement(), r)?;
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => w.place(
                    place,
                    AccessKindAttr::Write,
                    None,
                    PlaceAccessRequirementV1::IfMemory,
                    r,
                )?,
                SemanticStatementKindV1::Assume(v) => w.operand(v, r)?,
                SemanticStatementKindV1::StorageLive(local)
                | SemanticStatementKindV1::StorageDead(local) => {
                    if f.locals().get(local.index() as usize).is_none() {
                        return Err(ProductionRankedProjectionErrorV1::Incomplete(
                            "source occurrence oracle storage local absent",
                        ));
                    }
                }
                SemanticStatementKindV1::Nop => {}
            }
        }
        r.work(64)?;
        let mut w = Walk {
            block: bi,
            statement: None,
            ordinal: 0,
            source: block.terminator().source(),
            visit: &mut *visit,
            marker: std::marker::PhantomData,
        };
        match block.terminator().kind() {
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                w.operand(discriminant, r)?
            }
            SemanticTerminatorKindV1::Assert { condition, .. } => w.operand(condition, r)?,
            SemanticTerminatorKindV1::Call(_)
            | SemanticTerminatorKindV1::TailCall(_)
            | SemanticTerminatorKindV1::Drop { .. } => refused_call(
                Coordinate {
                    block: bi,
                    statement: None,
                    ordinal: 0,
                },
                r,
            )?,
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => {}
        }
    }
    Ok(())
}
