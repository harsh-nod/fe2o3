//! Source-checked, adjacent sole-use metadata carriers. The original reference
//! body hash retains the borrow/raw-fake definition; only its length observation
//! is represented in the scalar reference IR. Ordinary raw pointers stay closed.

use super::*;
use rustc_middle::mir::visit::{
    MutatingUseContext, NonMutatingUseContext, NonUseContext, PlaceContext, Visitor,
};
use rustc_middle::mir::{BorrowKind, Local, Location, RawPtrKind};

#[derive(Clone)]
struct Slot {
    definition: Location,
    consumption: Location,
    stores: u8,
    reads: u8,
}

#[derive(Default)]
pub(super) struct Metadata {
    slots: BTreeMap<Local, Slot>,
    definitions: BTreeSet<Location>,
    lengths: BTreeMap<Location, u32>,
}

impl Metadata {
    pub(super) fn is_carrier(&self, local: Local) -> bool {
        self.slots.contains_key(&local)
    }
    pub(super) fn is_definition(&self, location: Location) -> bool {
        self.definitions.contains(&location)
    }
    pub(super) fn length_source(&self, location: Location) -> Option<u32> {
        self.lengths.get(&location).copied()
    }
}

pub(super) fn collect<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    relations: &[ReferenceArgumentRelationV1],
    work: &mut ReferenceSymbolicWorkBudgetV2,
) -> Result<Metadata, ReferenceBindingErrorV1> {
    let mut metadata = Metadata::default();
    if !relations.iter().any(|r| {
        matches!(
            r,
            ReferenceArgumentRelationV1::InvocationDisjointOutputSlice1D { .. }
            | ReferenceArgumentRelationV1::ExclusivePrimitiveOutputSlice1D { .. }
        )
    }) {
        return Ok(metadata);
    }
    let fail = || {
        ReferenceBindingErrorV1::new(
            "output-slice metadata carrier is not an exact adjacent sole-use source borrow",
        )
    };
    work.charge_v2(relations.len())?;
    if relations.len() != body.arg_count
        || !matches!(
            relations.first(),
            Some(ReferenceArgumentRelationV1::PointCoordinate {
                reference_argument: 0,
                axis: 0
            })
        )
    {
        return Err(fail());
    }
    for (block, data) in body.basic_blocks.iter_enumerated() {
        for (statement_index, statement) in data.statements.iter().enumerate() {
            work.charge_v2(1)?;
            let StatementKind::Assign(assignment) = &statement.kind else {
                continue;
            };
            let (destination, value) = &**assignment;
            let (source, raw) = match value {
                Rvalue::Ref(_, BorrowKind::Shared, source) => (source, false),
                Rvalue::RawPtr(RawPtrKind::FakeForPtrMetadata, source) => (source, true),
                _ => continue,
            };
            let Some(reference_argument) = source.local.as_u32().checked_sub(1) else {
                continue;
            };
            let Some(ReferenceArgumentRelationV1::InvocationDisjointOutputSlice1D {
                element: expected,
                ..
            } | ReferenceArgumentRelationV1::ExclusivePrimitiveOutputSlice1D {
                element: expected, ..
            }) = relations.get(reference_argument as usize)
            else {
                continue;
            };
            let Some(local) = destination
                .as_local()
                .filter(|l| l.as_usize() > body.arg_count)
            else {
                return Err(fail());
            };
            let TyKind::Ref(_, pointee, Mutability::Mut) =
                *body.local_decls[source.local].ty.kind()
            else {
                return Err(fail());
            };
            let TyKind::Slice(element) = *pointee.kind() else {
                return Err(fail());
            };
            if scalar_type_v1(element) != Some(*expected)
                || !matches!(source.projection.as_ref(), [ProjectionElem::Deref])
            {
                return Err(fail());
            }
            let exact_type = if raw {
                matches!(*body.local_decls[local].ty.kind(), TyKind::RawPtr(actual, Mutability::Not) if actual == pointee)
            } else {
                matches!(*body.local_decls[local].ty.kind(), TyKind::Ref(_, actual, Mutability::Not) if actual == pointee)
            };
            let Some(next) = data.statements.get(statement_index + 1) else {
                return Err(fail());
            };
            let StatementKind::Assign(next) = &next.kind else {
                return Err(fail());
            };
            let (length, value) = &**next;
            if !exact_type
                || !length
                    .as_local()
                    .is_some_and(|l| l.as_usize() > body.arg_count && l != local)
                || length.ty(&body.local_decls, tcx).ty != tcx.types.usize
                || !matches!(value, Rvalue::UnaryOp(UnOp::PtrMetadata, Operand::Move(place)) if place.as_local() == Some(local))
            {
                return Err(fail());
            }
            let definition = Location {
                block,
                statement_index,
            };
            let consumption = Location {
                block,
                statement_index: statement_index + 1,
            };
            work.charge_v2(6)?;
            if metadata
                .slots
                .insert(
                    local,
                    Slot {
                        definition,
                        consumption,
                        stores: 0,
                        reads: 0,
                    },
                )
                .is_some()
                || !metadata.definitions.insert(definition)
                || metadata
                    .lengths
                    .insert(consumption, reference_argument)
                    .is_some()
            {
                return Err(fail());
            }
        }
    }
    let mut audit = Audit {
        slots: &mut metadata.slots,
        work,
        valid: true,
    };
    // Debug records do not execute. Visit every original statement/terminator,
    // including unreachable blocks and cleanup, with one shared work owner.
    for (block, data) in body.basic_blocks.iter_enumerated() {
        for (statement_index, statement) in data.statements.iter().enumerate() {
            audit.visit_statement(
                statement,
                Location {
                    block,
                    statement_index,
                },
            );
        }
        audit.visit_terminator(
            data.terminator(),
            Location {
                block,
                statement_index: data.statements.len(),
            },
        );
    }
    if !audit.valid || audit.slots.values().any(|s| s.stores != 1 || s.reads != 1) {
        return Err(fail());
    }
    Ok(metadata)
}

struct Audit<'a> {
    slots: &'a mut BTreeMap<Local, Slot>,
    work: &'a mut ReferenceSymbolicWorkBudgetV2,
    valid: bool,
}

impl Audit<'_> {
    fn charge(&mut self, amount: usize) -> bool {
        self.valid &= self.work.charge_v2(amount).is_ok();
        self.valid
    }
}

impl<'tcx> Visitor<'tcx> for Audit<'_> {
    fn visit_statement(
        &mut self,
        statement: &rustc_middle::mir::Statement<'tcx>,
        location: Location,
    ) {
        if self.charge(1) {
            self.super_statement(statement, location);
        }
    }
    fn visit_terminator(
        &mut self,
        terminator: &rustc_middle::mir::Terminator<'tcx>,
        location: Location,
    ) {
        let size = match &terminator.kind {
            TerminatorKind::Call { args, .. } => 1 + args.len(),
            TerminatorKind::SwitchInt { targets, .. } => 1 + targets.all_targets().len(),
            _ => 1,
        };
        if self.charge(size) {
            self.super_terminator(terminator, location);
        }
    }
    fn visit_rvalue(&mut self, value: &Rvalue<'tcx>, location: Location) {
        let size = match value {
            Rvalue::Aggregate(_, fields) => 1 + fields.len(),
            _ => 1,
        };
        if self.charge(size) {
            self.super_rvalue(value, location);
        }
    }
    fn visit_place(&mut self, place: &Place<'tcx>, context: PlaceContext, location: Location) {
        if !self.charge(1 + place.projection.len()) {
            return;
        }
        if self.slots.contains_key(&place.local) && !place.projection.is_empty() {
            self.valid = false;
            return;
        }
        self.super_place(place, context, location);
    }
    fn visit_local(&mut self, local: Local, context: PlaceContext, location: Location) {
        if !self.charge(1) {
            return;
        }
        let Some(slot) = self.slots.get_mut(&local) else {
            return;
        };
        if matches!(
            context,
            PlaceContext::NonUse(NonUseContext::StorageLive | NonUseContext::StorageDead)
        ) {
            return;
        }
        if location == slot.definition
            && context == PlaceContext::MutatingUse(MutatingUseContext::Store)
            && slot.stores == 0
        {
            slot.stores = 1;
        } else if location == slot.consumption
            && context == PlaceContext::NonMutatingUse(NonMutatingUseContext::Move)
            && slot.reads == 0
        {
            slot.reads = 1;
        } else {
            self.valid = false;
        }
    }
}

#[cfg(test)]
#[path = "output_slice_metadata_v1/tests.rs"]
mod tests;
