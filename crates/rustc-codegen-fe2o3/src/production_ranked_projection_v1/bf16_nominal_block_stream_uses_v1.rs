//! Original-order source uses consumed directly by the owning block emitter.
//! This visitor allocates no roster. It confers no source/admission authority.
use super::*;
use crate::production_ranked_projection_v1::root_checked_reference_use_preparation_v1::SourceUseOccurrenceV1;

pub(super) enum Use<'a> {
    Value(SourceUseOccurrenceV1<'a>),
    Address(&'a SemanticPlaceV1),
}
struct Visitor<'a, 'e, F> {
    site: ProjectedSemanticAccessSiteV1,
    source: SemanticSourceProvenanceV1,
    ordinal: usize,
    emit: &'e mut F,
    _source: std::marker::PhantomData<&'a SemanticFunctionDeclV1>,
}
impl<'a, F> Visitor<'a, '_, F>
where
    F: FnMut(Use<'a>, &mut PreparationResourcesV1<'_, '_>) -> Result<()>,
{
    fn place(
        &mut self,
        place: &'a SemanticPlaceV1,
        access: AccessKindAttr,
        atomic: Option<SemanticAtomicAccessV1>,
        requirement: PlaceAccessRequirementV1,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<()> {
        resources.work(32)?;
        let ordinal = self.ordinal;
        self.ordinal = ordinal
            .checked_add(1)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        (self.emit)(
            Use::Value(SourceUseOccurrenceV1 {
                place,
                site: self.site,
                ordinal,
                access,
                atomic,
                requirement,
                source: self.source,
            }),
            resources,
        )
    }
    fn operand(
        &mut self,
        operand: &'a SemanticOperandV1,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<()> {
        resources.work(8)?;
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => self.place(
                place,
                AccessKindAttr::Read,
                None,
                PlaceAccessRequirementV1::IfMemory,
                resources,
            ),
            SemanticOperandV1::Constant(_) => Ok(()),
        }
    }
    fn rvalue(
        &mut self,
        value: &'a SemanticRvalueKindV1,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<()> {
        resources.work(16)?;
        match value {
            SemanticRvalueKindV1::Use(operand)
            | SemanticRvalueKindV1::Unary { operand, .. }
            | SemanticRvalueKindV1::Cast { operand, .. } => self.operand(operand, resources),
            SemanticRvalueKindV1::Binary { left, right, .. } => {
                self.operand(left, resources)?;
                self.operand(right, resources)
            }
            SemanticRvalueKindV1::CheckedBinary(binary) => {
                self.operand(binary.left(), resources)?;
                self.operand(binary.right(), resources)
            }
            SemanticRvalueKindV1::UncheckedBinary(binary) => {
                self.operand(binary.left(), resources)?;
                self.operand(binary.right(), resources)
            }
            SemanticRvalueKindV1::Aggregate(aggregate) => {
                for operand in aggregate.operands() {
                    self.operand(operand, resources)?;
                }
                Ok(())
            }
            SemanticRvalueKindV1::Load(load) => self.place(
                load.source(),
                if load.atomic().is_some() {
                    AccessKindAttr::AtomicRead
                } else {
                    AccessKindAttr::Read
                },
                load.atomic(),
                PlaceAccessRequirementV1::ExplicitMemory,
                resources,
            ),
            SemanticRvalueKindV1::AddressOf { place, .. }
            | SemanticRvalueKindV1::Borrow { place, .. } => {
                (self.emit)(Use::Address(place), resources)
            }
            SemanticRvalueKindV1::Length(place) | SemanticRvalueKindV1::Discriminant(place) => self
                .place(
                    place,
                    AccessKindAttr::Read,
                    None,
                    PlaceAccessRequirementV1::IfMemory,
                    resources,
                ),
        }
    }
}

/// The caller authenticates each Call BEFORE choosing this visitor. No call is
/// assumed empty: arguments and destination still traverse the same callback.
/// TailCall and Drop preserve explicit refusal. Address formation is a separate
/// event, never silently omitted as it is in the diagnostic single-use selector.
pub(super) fn visit<'a, F>(
    function: &'a SemanticFunctionDeclV1,
    site: ProjectedSemanticAccessSiteV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
    mut emit: F,
) -> Result<usize>
where
    F: FnMut(Use<'a>, &mut PreparationResourcesV1<'_, '_>) -> Result<()>,
{
    resources.work(64)?;
    if !resources.is_metered() || resources.has_denial() {
        return Err(resource(Resource::Accounting));
    }
    let block = function
        .blocks()
        .get(site.block)
        .ok_or(Error::Incomplete("block stream source block absent"))?;
    let statement = match site.statement {
        Some(i) => Some(
            block
                .statements()
                .get(i)
                .ok_or(Error::Incomplete("block stream source statement absent"))?,
        ),
        None => None,
    };
    let mut visitor = Visitor {
        site,
        source: statement.map_or_else(|| block.terminator().source(), |s| s.source()),
        ordinal: 0,
        emit: &mut emit,
        _source: std::marker::PhantomData,
    };
    if let Some(statement) = statement {
        match statement.kind() {
            SemanticStatementKindV1::Assign(assignment) => {
                visitor.rvalue(assignment.value().kind(), resources)?;
                visitor.place(
                    assignment.destination(),
                    AccessKindAttr::Write,
                    None,
                    PlaceAccessRequirementV1::IfMemory,
                    resources,
                )?;
            }
            SemanticStatementKindV1::Store(store) => {
                visitor.operand(store.value(), resources)?;
                visitor.place(
                    store.destination(),
                    if store.atomic().is_some() {
                        AccessKindAttr::AtomicWrite
                    } else {
                        AccessKindAttr::Write
                    },
                    store.atomic(),
                    PlaceAccessRequirementV1::ExplicitMemory,
                    resources,
                )?;
            }
            SemanticStatementKindV1::AtomicRmw(atomic) => {
                visitor.operand(atomic.value(), resources)?;
                visitor.place(
                    atomic.address(),
                    AccessKindAttr::AtomicReadModifyWrite,
                    Some(atomic.access()),
                    PlaceAccessRequirementV1::ExplicitMemory,
                    resources,
                )?;
                visitor.place(
                    atomic.destination(),
                    AccessKindAttr::Write,
                    None,
                    PlaceAccessRequirementV1::IfMemory,
                    resources,
                )?;
            }
            SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                visitor.place(
                    atomic.destination(),
                    AccessKindAttr::Write,
                    None,
                    PlaceAccessRequirementV1::IfMemory,
                    resources,
                )?;
                visitor.place(
                    atomic.address(),
                    AccessKindAttr::AtomicReadModifyWrite,
                    Some(atomic.success()),
                    PlaceAccessRequirementV1::ExplicitMemory,
                    resources,
                )?;
                visitor.operand(atomic.expected(), resources)?;
                visitor.operand(atomic.replacement(), resources)?;
            }
            SemanticStatementKindV1::SetDiscriminant { place, .. }
            | SemanticStatementKindV1::Deinitialize(place) => visitor.place(
                place,
                AccessKindAttr::Write,
                None,
                PlaceAccessRequirementV1::IfMemory,
                resources,
            )?,
            SemanticStatementKindV1::StorageLive(local)
            | SemanticStatementKindV1::StorageDead(local) => {
                if function.locals().get(local.index() as usize).is_none() {
                    return Err(Error::Unsupported(
                        "a storage statement with an out-of-range local",
                    ));
                }
            }
            SemanticStatementKindV1::Assume(condition) => visitor.operand(condition, resources)?,
            SemanticStatementKindV1::Nop => {}
        }
    } else {
        match block.terminator().kind() {
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                visitor.operand(discriminant, resources)?
            }
            SemanticTerminatorKindV1::Assert { condition, .. } => {
                visitor.operand(condition, resources)?
            }
            SemanticTerminatorKindV1::Call(call) => {
                for argument in call.arguments() {
                    visitor.operand(argument, resources)?;
                }
                if let Some(destination) = call.destination() {
                    visitor.place(
                        destination.place(),
                        AccessKindAttr::Write,
                        None,
                        PlaceAccessRequirementV1::IfMemory,
                        resources,
                    )?;
                }
            }
            SemanticTerminatorKindV1::TailCall(_) => {
                return Err(Error::Incomplete(
                    "nominal block stream does not admit tail calls",
                ));
            }
            SemanticTerminatorKindV1::Drop { .. } => {
                return Err(Error::Incomplete(
                    "nominal block stream retains unresolved Drop effects",
                ));
            }
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => {}
        }
    }
    Ok(visitor.ordinal)
}
