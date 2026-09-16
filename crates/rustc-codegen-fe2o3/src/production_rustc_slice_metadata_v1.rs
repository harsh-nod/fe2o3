//! Exact elimination of rustc's metadata-only slice pointer temporaries.

use rustc_hir::Mutability;
use rustc_middle::mir::visit::{PlaceContext, Visitor};
use rustc_middle::mir::{
    Body, Local, Location, Operand, Place, ProjectionElem, RawPtrKind, Rvalue, Statement,
    StatementKind, StmtDebugInfo, Terminator, TerminatorKind, UnOp,
};
use rustc_middle::ty::{EarlyBinder, Instance, Ty, TyCtxt, TyKind, TypingEnv, UintTy};

#[derive(Debug)]
pub(crate) enum SliceMetadataErrorV1<E> {
    Unsupported(Location),
    Allocation,
    Resource(E),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MetadataPairV1 {
    producer: Location,
    temporary: Local,
    slice: Local,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SliceMetadataRewriteV1 {
    ElideTemporary,
    ReadLength(Local),
}

/// Borrows the exact unmodified MIR whose complete temporary uses were checked.
pub(crate) struct SliceMetadataPlanV1<'a, 'tcx> {
    _body: &'a Body<'tcx>,
    pairs: Vec<MetadataPairV1>,
}

impl<'a, 'tcx> SliceMetadataPlanV1<'a, 'tcx> {
    pub(crate) fn derive<E>(
        tcx: TyCtxt<'tcx>,
        instance: Instance<'tcx>,
        body: &'a Body<'tcx>,
        mut charge: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, SliceMetadataErrorV1<E>> {
        let mut pairs = Vec::new();
        for (block, data) in body.basic_blocks.iter_enumerated() {
            charge(1).map_err(SliceMetadataErrorV1::Resource)?;
            for (statement_index, statement) in data.statements.iter().enumerate() {
                charge(1).map_err(SliceMetadataErrorV1::Resource)?;
                let StatementKind::Assign(assignment) = &statement.kind else {
                    continue;
                };
                if !matches!(
                    assignment.1,
                    Rvalue::RawPtr(RawPtrKind::FakeForPtrMetadata, _)
                ) {
                    continue;
                }
                let producer = Location {
                    block,
                    statement_index,
                };
                charge(4).map_err(SliceMetadataErrorV1::Resource)?;
                let pair = metadata_pair(tcx, instance, body, producer)
                    .ok_or(SliceMetadataErrorV1::Unsupported(producer))?;
                pairs
                    .try_reserve(1)
                    .map_err(|_| SliceMetadataErrorV1::Allocation)?;
                pairs.push(pair);
            }
        }
        if !pairs.is_empty() {
            charge(body.local_decls.len()).map_err(SliceMetadataErrorV1::Resource)?;
            let mut uses = Vec::new();
            uses.try_reserve_exact(body.local_decls.len())
                .map_err(|_| SliceMetadataErrorV1::Allocation)?;
            uses.resize(body.local_decls.len(), 0_u8);
            let mut visitor = TemporaryUsesV1 {
                uses: &mut uses,
                charge: &mut charge,
                error: None,
            };
            // Include unreachable blocks and every executable place use. Debug
            // observations retain their type/identity but gain no runtime value.
            for (block, data) in body.basic_blocks.iter_enumerated() {
                for (statement_index, statement) in data.statements.iter().enumerate() {
                    if visitor.work(1)
                        && !matches!(
                            statement.kind,
                            StatementKind::StorageLive(_) | StatementKind::StorageDead(_)
                        )
                    {
                        visitor.visit_statement(
                            statement,
                            Location {
                                block,
                                statement_index,
                            },
                        );
                    }
                    if let Some(error) = visitor.error.take() {
                        return Err(error);
                    }
                }
                if let Some(terminator) = &data.terminator {
                    if visitor.work(1) {
                        visitor.visit_terminator(
                            terminator,
                            Location {
                                block,
                                statement_index: data.statements.len(),
                            },
                        );
                    }
                    if let Some(error) = visitor.error.take() {
                        return Err(error);
                    }
                }
            }
            for pair in &pairs {
                charge(1).map_err(SliceMetadataErrorV1::Resource)?;
                if uses[pair.temporary.index()] != 2 {
                    return Err(SliceMetadataErrorV1::Unsupported(pair.producer));
                }
            }
        }
        Ok(Self { _body: body, pairs })
    }

    pub(crate) fn lookup_work(&self) -> usize {
        self.pairs
            .len()
            .checked_ilog2()
            .map_or(0, |depth| 2 * (depth as usize + 2))
    }

    pub(crate) fn at(&self, location: Location) -> Option<SliceMetadataRewriteV1> {
        let find = |location: Location| {
            self.pairs
                .binary_search_by_key(
                    &(location.block.index(), location.statement_index),
                    |pair| (pair.producer.block.index(), pair.producer.statement_index),
                )
                .ok()
                .map(|index| self.pairs[index])
        };
        if find(location).is_some() {
            return Some(SliceMetadataRewriteV1::ElideTemporary);
        }
        let previous = Location {
            statement_index: location.statement_index.checked_sub(1)?,
            ..location
        };
        find(previous).map(|pair| SliceMetadataRewriteV1::ReadLength(pair.slice))
    }
}

fn metadata_pair<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    producer: Location,
) -> Option<MetadataPairV1> {
    let statements = &body.basic_blocks[producer.block].statements;
    let StatementKind::Assign(assignment) = &statements.get(producer.statement_index)?.kind else {
        return None;
    };
    let temporary = assignment.0.as_local()?;
    if temporary.index() <= body.arg_count {
        return None;
    }
    let Rvalue::RawPtr(RawPtrKind::FakeForPtrMetadata, place) = assignment.1 else {
        return None;
    };
    if !matches!(place.projection.as_ref(), [ProjectionElem::Deref]) {
        return None;
    }
    let StatementKind::Assign(consumer) = &statements
        .get(producer.statement_index.checked_add(1)?)?
        .kind
    else {
        return None;
    };
    let length = consumer.0.as_local()?;
    let Rvalue::UnaryOp(UnOp::PtrMetadata, Operand::Copy(pointer) | Operand::Move(pointer)) =
        &consumer.1
    else {
        return None;
    };
    if pointer.as_local() != Some(temporary) {
        return None;
    }
    let normalize = |local: Local| -> Option<Ty<'tcx>> {
        instance
            .try_instantiate_mir_and_normalize_erasing_regions(
                tcx,
                TypingEnv::fully_monomorphized(),
                EarlyBinder::bind(body.local_decls.get(local)?.ty),
            )
            .ok()
    };
    let slice = normalize(place.local)?;
    let TyKind::Ref(_, pointee, Mutability::Not) = *slice.kind() else {
        return None;
    };
    if !matches!(pointee.kind(), TyKind::Slice(element) if matches!(element.kind(), TyKind::Uint(UintTy::U32)))
    {
        return None;
    }
    if !matches!(normalize(temporary)?.kind(), TyKind::RawPtr(raw, Mutability::Not) if *raw == pointee)
        || normalize(length)? != tcx.types.usize
    {
        return None;
    }
    Some(MetadataPairV1 {
        producer,
        temporary,
        slice: place.local,
    })
}

struct TemporaryUsesV1<'a, E, F> {
    uses: &'a mut [u8],
    charge: &'a mut F,
    error: Option<SliceMetadataErrorV1<E>>,
}

impl<E, F: FnMut(usize) -> Result<(), E>> TemporaryUsesV1<'_, E, F> {
    fn work(&mut self, amount: usize) -> bool {
        if self.error.is_none() {
            self.error = (self.charge)(amount)
                .err()
                .map(SliceMetadataErrorV1::Resource);
        }
        self.error.is_none()
    }
}

impl<'tcx, E, F: FnMut(usize) -> Result<(), E>> Visitor<'tcx> for TemporaryUsesV1<'_, E, F> {
    fn visit_statement(&mut self, statement: &Statement<'tcx>, location: Location) {
        if self.work(statement.debuginfos.len()) {
            self.super_statement(statement, location);
        }
    }

    fn visit_statement_debuginfo(&mut self, _: &StmtDebugInfo<'tcx>, _: Location) {}

    fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
        let children = match &terminator.kind {
            TerminatorKind::Call { args, .. } | TerminatorKind::TailCall { args, .. } => args.len(),
            TerminatorKind::InlineAsm { operands, .. } => operands.len(),
            _ => 0,
        };
        if self.work(children) {
            self.super_terminator(terminator, location);
        }
    }

    fn visit_local(&mut self, local: Local, _: PlaceContext, location: Location) {
        if !self.work(1) {
            return;
        }
        if let Some(count) = self.uses.get_mut(local.index()) {
            *count = count.saturating_add(1).min(3);
        } else {
            self.error = Some(SliceMetadataErrorV1::Unsupported(location));
        }
    }

    fn visit_place(&mut self, place: &Place<'tcx>, context: PlaceContext, location: Location) {
        if self.work(place.projection.len().saturating_add(1)) {
            self.super_place(place, context, location);
        }
    }

    fn visit_operand(&mut self, operand: &Operand<'tcx>, location: Location) {
        if self.work(1) {
            self.super_operand(operand, location);
        }
    }

    fn visit_rvalue(&mut self, value: &Rvalue<'tcx>, location: Location) {
        let children = match value {
            Rvalue::Aggregate(_, operands) => operands.len(),
            _ => 0,
        };
        // Prepay collection traversal before rustc's visitor enters a loop,
        // including iterations remaining after a nested callback rejects.
        if self.work(children.saturating_add(1)) {
            self.super_rvalue(value, location);
        }
    }
}

#[cfg(test)]
mod tests;
