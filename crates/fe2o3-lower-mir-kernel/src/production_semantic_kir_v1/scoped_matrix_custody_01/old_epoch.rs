//! Rejection-only closure over actual old-scope types. This cannot authorize a
//! use, infer an issuer, or turn a scalar/constant into a capability value.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticAssertMessageV1;

mod inventory;
mod reverse_types;
pub(super) use inventory::Index;

pub(super) fn reject_uses<'a>(
    owner: &'a ProductionSemanticSsaOwnerV1,
    view: &'a SemanticExpandedRootV1,
    graph: &mut Graph<'a>,
    after: u32,
    seeds: &[SemanticTypeIdV1],
    index: &mut Option<Index<'a>>,
) -> Result<()> {
    if !std::ptr::eq(graph.body, view.body()) {
        return Err(mismatch());
    }
    let declarations = owner.source_semantic().types();
    if index.is_none() {
        *index = Some(Index::new(declarations, graph.body, &mut |n| graph.charge(n))?);
    }
    if let Some((block, statement)) = index.as_mut().ok_or_else(mismatch)?
        .first_use(declarations, graph, after, seeds)? {
        let source = view
            .block_origins()
            .get(block as usize)
            .ok_or_else(mismatch)?;
        return Err(unsupported(
            source.function().index(),
            Some(source.block().index()),
            statement,
            "transpose Publish has a subsequent old-epoch or unmodeled descendant use",
        ));
    }
    Ok(())
}

#[cfg(test)]
fn first_use(
    declarations: &[SemanticTypeDeclV1],
    graph: &mut Graph<'_>,
    after: u32,
    seeds: &[SemanticTypeIdV1],
) -> Result<Option<(u32, Option<u32>)>> {
    graph.charge(12 + seeds.len())?;
    if graph.body.blocks().get(after as usize).is_none()
        || seeds.is_empty()
        || seeds
            .iter()
            .any(|ty| declarations.get(ty.index() as usize).is_none())
    {
        return Err(mismatch());
    }
    let mut types = Types {
        types: declarations,
        seeds,
        complete: BTreeMap::new(),
    };
    // Only candidate blocks need a CFG query. The existing graph supplies exact
    // reachable-path semantics and charges every cold or cached query.
    let body = graph.body;
    for (index, block) in body.blocks().iter().enumerate() {
        graph.charge(1)?;
        let mut candidate = None;
        for (statement, value) in block.statements().iter().enumerate() {
            graph.charge(1)?;
            if types.statement(body, value.kind(), &mut |n| graph.charge(n))? {
                candidate = Some(Some(statement as u32));
                break;
            }
        }
        if candidate.is_none()
            && types.terminator(body, block.terminator().kind(), &mut |n| graph.charge(n))?
        {
            candidate = Some(None);
        }
        if let Some(statement) = candidate {
            if graph.reaches(after, index as u32)? {
                return Ok(Some((index as u32, statement)));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
struct Types<'a> {
    types: &'a [SemanticTypeDeclV1],
    seeds: &'a [SemanticTypeIdV1],
    complete: BTreeMap<SemanticTypeIdV1, bool>,
}

#[cfg(test)]
impl Types<'_> {
    fn contains(
        &mut self,
        start: SemanticTypeIdV1,
        charge: &mut dyn FnMut(usize) -> Result<()>,
    ) -> Result<bool> {
        charge(1 + (usize::BITS - self.complete.len().leading_zeros()) as usize)?;
        if let Some(found) = self.complete.get(&start) {
            return Ok(*found);
        }
        // Entries are memoized ONLY for a completed root query. A cycle hit is
        // never saved as false for an intermediate type, including SCC peers.
        charge(12)?;
        let mut visited = BTreeSet::new();
        let mut pending = Vec::new();
        push(&mut pending, start, charge)?;
        let mut found = false;
        while let Some(ty) = pending.pop() {
            charge(1 + self.seeds.len() + (usize::BITS - visited.len().leading_zeros()) as usize)?;
            if self.seeds.contains(&ty) {
                found = true;
                break;
            }
            if visited.contains(&ty) {
                continue;
            }
            charge(8)?;
            visited.insert(ty);
            let shape = self
                .types
                .get(ty.index() as usize)
                .ok_or_else(mismatch)?
                .shape();
            match shape {
                SemanticTypeShapeV1::Pointer(pointer) => {
                    push(&mut pending, pointer.pointee(), charge)?
                }
                SemanticTypeShapeV1::Array { element, .. }
                | SemanticTypeShapeV1::Slice { element } => push(&mut pending, *element, charge)?,
                SemanticTypeShapeV1::Tuple(fields)
                | SemanticTypeShapeV1::Aggregate(fields)
                | SemanticTypeShapeV1::Union(fields) => {
                    for &field in fields.fields() {
                        push(&mut pending, field, charge)?;
                    }
                }
                SemanticTypeShapeV1::Enum {
                    discriminant,
                    variants,
                } => {
                    push(&mut pending, *discriminant, charge)?;
                    for variant in variants {
                        for &field in variant.fields().fields() {
                            push(&mut pending, field, charge)?;
                        }
                    }
                }
                SemanticTypeShapeV1::FunctionPointer {
                    arguments,
                    return_type,
                    ..
                } => {
                    push(&mut pending, *return_type, charge)?;
                    for &argument in arguments.fields() {
                        push(&mut pending, argument, charge)?;
                    }
                }
                // Opaque storage cannot establish absence of an old-scope
                // descendant. Mark it as a rejection candidate; the caller
                // still checks that the use is reachable after Publish.
                SemanticTypeShapeV1::Opaque => {
                    found = true;
                    break;
                }
                SemanticTypeShapeV1::Unit
                | SemanticTypeShapeV1::Never
                | SemanticTypeShapeV1::Scalar(_)
                | SemanticTypeShapeV1::ValidityScalar(_) => {}
            }
        }
        charge(8 + (usize::BITS - self.complete.len().leading_zeros()) as usize)?;
        self.complete.insert(start, found);
        Ok(found)
    }
}

// One exhaustive source visitor serves the classifier and its dependency
// census. The census returns false to visit every disjunct, never a proof.
trait TypeUses {
    fn contains(
        &mut self,
        ty: SemanticTypeIdV1,
        charge: &mut dyn FnMut(usize) -> Result<()>,
    ) -> Result<bool>;

    fn operand(
        &mut self,
        body: &SemanticFunctionDeclV1,
        operand: &SemanticOperandV1,
        charge: &mut dyn FnMut(usize) -> Result<()>,
    ) -> Result<bool> {
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                self.place(body, place, charge)
            }
            SemanticOperandV1::Constant(_) => self.contains(operand.ty(), charge),
        }
    }

    fn place(
        &mut self,
        body: &SemanticFunctionDeclV1,
        place: &SemanticPlaceV1,
        charge: &mut dyn FnMut(usize) -> Result<()>,
    ) -> Result<bool> {
        // Conservative about projected reads from a carrier holding an old
        // capability. No fresh storage-generation or field-disjointness theorem
        // is invented here. Unrelated scalar locals stay admissible.
        Ok(self.contains(place.ty(), charge)?
            || self.contains(
                body.locals()
                    .get(place.local().index() as usize)
                    .ok_or_else(mismatch)?
                    .ty(),
                charge,
            )?)
    }

    fn statement(
        &mut self,
        body: &SemanticFunctionDeclV1,
        statement: &SemanticStatementKindV1,
        charge: &mut dyn FnMut(usize) -> Result<()>,
    ) -> Result<bool> {
        Ok(match statement {
            SemanticStatementKindV1::Assign(a) => {
                let mut old = self.contains(a.value().result_type(), charge)?
                    || !a.destination().projections().is_empty()
                        && self.place(body, a.destination(), charge)?;
                a.value()
                    .kind()
                    .try_visit_operands::<ProductionSemanticKirErrorV1>(|operand| {
                        old |= self.operand(body, operand, charge)?;
                        Ok(())
                    })?;
                old || match a.value().kind() {
                    SemanticRvalueKindV1::Borrow { place, .. }
                    | SemanticRvalueKindV1::AddressOf { place, .. }
                    | SemanticRvalueKindV1::Length(place)
                    | SemanticRvalueKindV1::Discriminant(place) => {
                        self.place(body, place, charge)?
                    }
                    SemanticRvalueKindV1::Load(load) => self.place(body, load.source(), charge)?,
                    SemanticRvalueKindV1::Use(_)
                    | SemanticRvalueKindV1::Unary { .. }
                    | SemanticRvalueKindV1::Binary { .. }
                    | SemanticRvalueKindV1::CheckedBinary(_)
                    | SemanticRvalueKindV1::UncheckedBinary(_)
                    | SemanticRvalueKindV1::Cast { .. }
                    | SemanticRvalueKindV1::Aggregate(_) => false,
                }
            }
            SemanticStatementKindV1::Store(store) => {
                self.place(body, store.destination(), charge)?
                    || self.operand(body, store.value(), charge)?
            }
            SemanticStatementKindV1::AtomicRmw(atomic) => {
                self.place(body, atomic.destination(), charge)?
                    || self.place(body, atomic.address(), charge)?
                    || self.operand(body, atomic.value(), charge)?
            }
            SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                self.place(body, atomic.destination(), charge)?
                    || self.place(body, atomic.address(), charge)?
                    || self.operand(body, atomic.expected(), charge)?
                    || self.operand(body, atomic.replacement(), charge)?
            }
            SemanticStatementKindV1::Assume(operand) => self.operand(body, operand, charge)?,
            SemanticStatementKindV1::SetDiscriminant { place, .. } => {
                self.place(body, place, charge)?
            }
            // Ending storage after the last use is not a use of its value.
            SemanticStatementKindV1::StorageLive(_)
            | SemanticStatementKindV1::StorageDead(_)
            | SemanticStatementKindV1::Deinitialize(_)
            | SemanticStatementKindV1::Nop => false,
        })
    }

    fn terminator(
        &mut self,
        body: &SemanticFunctionDeclV1,
        terminator: &SemanticTerminatorKindV1,
        charge: &mut dyn FnMut(usize) -> Result<()>,
    ) -> Result<bool> {
        Ok(match terminator {
            SemanticTerminatorKindV1::Call(call) => {
                self.operands(body, call.arguments(), charge)?
                    || match call.destination() {
                        Some(destination) => self.place(body, destination.place(), charge)?,
                        None => false,
                    }
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                self.operands(body, call.arguments(), charge)?
                    || self.contains(body.abi().source_output_type(), charge)?
            }
            SemanticTerminatorKindV1::Drop { place, .. } => self.place(body, place, charge)?,
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                self.operand(body, discriminant, charge)?
            }
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => {
                self.operand(body, condition, charge)?
                    || match message {
                        SemanticAssertMessageV1::BoundsCheck { length, index } => {
                            self.operand(body, length, charge)?
                                || self.operand(body, index, charge)?
                        }
                        SemanticAssertMessageV1::Overflow { left, right, .. } => {
                            self.operand(body, left, charge)?
                                || self.operand(body, right, charge)?
                        }
                        SemanticAssertMessageV1::DivisionByZero(value)
                        | SemanticAssertMessageV1::RemainderByZero(value) => {
                            self.operand(body, value, charge)?
                        }
                        SemanticAssertMessageV1::MisalignedPointerDereference {
                            required_alignment,
                            found_alignment,
                        } => {
                            self.operand(body, required_alignment, charge)?
                                || self.operand(body, found_alignment, charge)?
                        }
                        SemanticAssertMessageV1::NullPointerDereference
                        | SemanticAssertMessageV1::ResumedAfterReturn
                        | SemanticAssertMessageV1::ResumedAfterPanic => false,
                    }
            }
            SemanticTerminatorKindV1::Return => {
                self.contains(body.abi().source_output_type(), charge)?
            }
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => false,
        })
    }

    fn operands(
        &mut self,
        body: &SemanticFunctionDeclV1,
        operands: &[SemanticOperandV1],
        charge: &mut dyn FnMut(usize) -> Result<()>,
    ) -> Result<bool> {
        for operand in operands {
            charge(1)?;
            if self.operand(body, operand, charge)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
fn push(
    pending: &mut Vec<SemanticTypeIdV1>,
    value: SemanticTypeIdV1,
    charge: &mut dyn FnMut(usize) -> Result<()>,
) -> Result<()> {
    charge(2)?;
    pending
        .try_reserve_exact(1)
        .map_err(|_| ProductionSemanticKirErrorV1::AllocationFailure {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
        })?;
    pending.push(value);
    Ok(())
}

#[cfg(test)]
impl TypeUses for Types<'_> {
    fn contains(
        &mut self,
        ty: SemanticTypeIdV1,
        charge: &mut dyn FnMut(usize) -> Result<()>,
    ) -> Result<bool> {
        Types::contains(self, ty, charge)
    }
}

#[cfg(test)]
#[path = "old_epoch_tests.rs"]
mod tests;
