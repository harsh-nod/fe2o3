//! Exact shared primitive capture origins for the existing range evaluator.
//! This returns an operand origin, never an arithmetic or memory-effect waiver.

use super::*;

const MAX_LINKS: usize = 16;
const MAX_SEGMENT: usize = 16;
const MAX_FIELDS: usize = 16;

#[derive(Clone, Copy)]
struct Path {
    local: usize,
    field: Option<u32>,
    ty: SemanticTypeIdV1,
}

#[derive(Clone, Copy)]
struct Link {
    path: Path,
    definition: ScalarAssignmentSiteV1,
    consumer: ScalarAssignmentSiteV1,
}

fn same_site(left: ScalarAssignmentSiteV1, right: ScalarAssignmentSiteV1) -> bool {
    left.block == right.block && left.statement == right.statement
}

impl SemanticAssertProofsV1<'_> {
    pub(super) fn shared_capture_range_at_operand_v1(
        &mut self,
        operand: &SemanticOperandV1,
        site: ScalarAssignmentSiteV1,
    ) -> Result<Option<UnsignedRangeProofV1>, ProductionRankedProjectionErrorV1> {
        let previous = self.shared_capture_precision;
        self.shared_capture_precision = true;
        let result = self.range_at_operand(operand, site.block, site.statement);
        self.shared_capture_precision = previous;
        result
    }

    pub(super) fn shared_capture_candidate_v1(&self, place: &SemanticPlaceV1) -> bool {
        matches!(place.projections(), [projection] if projection.kind() == SemanticProjectionKindV1::Dereference && projection.result_type() == place.ty())
            && self.capture_unsigned_type_v1(place.ty())
            && self
                .function
                .locals()
                .get(place.local().index() as usize)
                .is_some_and(|local| self.capture_shared_reference_v1(local.ty(), place.ty()))
    }

    pub(super) fn shared_scalar_read_source_v1(
        &mut self,
        read: &SemanticPlaceV1,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<
        Option<(SemanticOperandV1, ScalarAssignmentSiteV1)>,
        ProductionRankedProjectionErrorV1,
    > {
        self.charge(1)?;
        if !self.function.blocks().get(use_site.block)
            .and_then(|block| block.statements().get(use_site.statement))
            .is_some_and(|statement| matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                if assignment.destination().projections().is_empty()
                    && assignment.destination().ty() == read.ty()
                    && assignment.value().result_type() == read.ty()
                    && matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) if place == read)))
        { return Ok(None); }
        let [projection] = read.projections() else {
            return Ok(None);
        };
        if projection.kind() != SemanticProjectionKindV1::Dereference
            || projection.result_type() != read.ty()
            || !self.capture_unsigned_type_v1(read.ty())
        {
            return Ok(None);
        }
        let Some(base) = self.function.locals().get(read.local().index() as usize) else {
            return Ok(None);
        };
        let mut path = Path {
            local: read.local().index() as usize,
            field: None,
            ty: base.ty(),
        };
        if !self.capture_shared_reference_v1(path.ty, read.ty()) {
            return Ok(None);
        }
        let mut consumer = use_site;
        let mut links = [None; MAX_LINKS];
        let mut length = 0;
        loop {
            self.charge(1)?;
            if length == MAX_LINKS
                || links[..length]
                    .iter()
                    .flatten()
                    .any(|link: &Link| link.path.local == path.local)
                || self.definition_counts.get(path.local).copied() != Some(1)
                || self.address_escaped.get(path.local).copied() != Some(false)
                || !self.capture_path_type_v1(path)
            {
                return Ok(None);
            }
            let Some(definition) = self.exact_reaching_assignment_v1(path.local, consumer)? else {
                return Ok(None);
            };
            let SemanticStatementKindV1::Assign(assignment) =
                self.function.blocks()[definition.block].statements()[definition.statement].kind()
            else {
                return Ok(None);
            };
            let local_ty = self.function.locals()[path.local].ty();
            if assignment.destination().ty() != local_ty
                || assignment.value().result_type() != local_ty
                || !assignment.destination().projections().is_empty()
                || assignment.destination().local().index() as usize != path.local
            {
                return Ok(None);
            }
            links[length] = Some(Link {
                path,
                definition,
                consumer,
            });
            length += 1;
            match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => {
                    let Some(next) = self.capture_operand_path_v1(operand) else {
                        return Ok(None);
                    };
                    if next.ty != local_ty || (path.field.is_some() && next.field.is_some()) {
                        return Ok(None);
                    }
                    path = Path {
                        local: next.local,
                        field: path.field.or(next.field),
                        ty: path.ty,
                    };
                    consumer = definition;
                }
                SemanticRvalueKindV1::Aggregate(aggregate) => {
                    let Some(field) = path.field else {
                        return Ok(None);
                    };
                    let Some(fields) = self.capture_fields_v1(local_ty) else {
                        return Ok(None);
                    };
                    if fields.len() > MAX_FIELDS
                        || aggregate.operands().len() != fields.len()
                        || !matches!(
                            (
                                self.types[local_ty.index() as usize].shape(),
                                aggregate.kind()
                            ),
                            (
                                SemanticTypeShapeV1::Tuple(_),
                                SemanticAggregateKindV1::Tuple
                            ) | (
                                SemanticTypeShapeV1::Aggregate(_),
                                SemanticAggregateKindV1::Aggregate
                            )
                        )
                    {
                        return Ok(None);
                    }
                    let Some(operand) = aggregate.operands().get(field as usize) else {
                        return Ok(None);
                    };
                    let Some(next) = self.capture_operand_path_v1(operand) else {
                        return Ok(None);
                    };
                    if next.ty != path.ty {
                        return Ok(None);
                    }
                    path = next;
                    consumer = definition;
                }
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place,
                } if path.field.is_none()
                    && place.projections().is_empty()
                    && place.ty() == read.ty()
                    && self.capture_shared_reference_v1(path.ty, read.ty()) =>
                {
                    let source = place.local().index() as usize;
                    let Some(declaration) = self.function.locals().get(source) else {
                        return Ok(None);
                    };
                    if declaration.ty() != read.ty()
                        || matches!(declaration.role(), SemanticLocalRoleV1::Argument(_))
                        || self.definition_counts.get(source).copied() != Some(1)
                        || self.address_escaped.get(source).copied() != Some(false)
                        || links[..length]
                            .iter()
                            .flatten()
                            .any(|link| link.path.local == source)
                    {
                        return Ok(None);
                    }
                    let Some(source_definition) =
                        self.exact_reaching_assignment_v1(source, definition)?
                    else {
                        return Ok(None);
                    };
                    if !self.capture_source_custody_v1(source, source_definition, definition)?
                        || !self.capture_segment_v1(
                            source,
                            &links[..length],
                            definition,
                            use_site,
                        )?
                    {
                        return Ok(None);
                    }
                    return Ok(Some((SemanticOperandV1::Copy(place.clone()), definition)));
                }
                _ => return Ok(None),
            }
        }
    }

    fn capture_unsigned_type_v1(&self, ty: SemanticTypeIdV1) -> bool {
        self.types.get(ty.index() as usize).is_some_and(|declaration| {
            matches!(declaration.shape(), SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed: false, bits: bits @ (8 | 16 | 32 | 64 | 128) })
                if declaration.layout().size_bytes() == Some(u64::from(*bits / 8)) && !declaration.layout().is_uninhabited())
        })
    }

    fn capture_shared_reference_v1(
        &self,
        reference: SemanticTypeIdV1,
        scalar: SemanticTypeIdV1,
    ) -> bool {
        self.types
            .get(reference.index() as usize)
            .is_some_and(|declaration| {
                matches!(declaration.shape(), SemanticTypeShapeV1::Pointer(pointer)
                if pointer.kind() == SemanticPointerKindV1::Reference
                    && pointer.mutability() == SemanticMutabilityV1::Immutable
                    && pointer.metadata() == SemanticPointerMetadataV1::None
                    && pointer.address_space() == 0 && pointer.pointer_width_bits() == 64
                    && pointer.pointee() == scalar && declaration.layout().size_bytes() == Some(8))
            })
    }

    fn capture_fields_v1(&self, ty: SemanticTypeIdV1) -> Option<&[SemanticTypeIdV1]> {
        match self.types.get(ty.index() as usize)?.shape() {
            SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                Some(fields.fields())
            }
            _ => None,
        }
    }

    fn capture_path_type_v1(&self, path: Path) -> bool {
        let Some(local) = self.function.locals().get(path.local) else {
            return false;
        };
        match path.field {
            None => local.ty() == path.ty,
            Some(field) => self.capture_fields_v1(local.ty()).is_some_and(|fields| {
                fields.len() <= MAX_FIELDS && fields.get(field as usize) == Some(&path.ty)
            }),
        }
    }

    fn capture_operand_path_v1(&self, operand: &SemanticOperandV1) -> Option<Path> {
        let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
            return None;
        };
        let field = match place.projections() {
            [] => None,
            [projection] if projection.result_type() == place.ty() => match projection.kind() {
                SemanticProjectionKindV1::Field(field) => Some(field),
                _ => return None,
            },
            _ => return None,
        };
        let path = Path {
            local: place.local().index() as usize,
            field,
            ty: place.ty(),
        };
        self.capture_path_type_v1(path).then_some(path)
    }

    // The referent has one exact definition and no other address producer.
    // Lifetime kills that can reach the borrow are rejected, including kills
    // on an earlier loop iteration. Later frame teardown is left intact.
    fn capture_source_custody_v1(
        &mut self,
        source: usize,
        definition: ScalarAssignmentSiteV1,
        borrow: ScalarAssignmentSiteV1,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        let reaching = self.blocks_reaching(borrow.block)?;
        for (bb, block) in self.function.blocks().iter().enumerate() {
            self.charge(1)?;
            let mut omitted_entry = false;
            block.terminator().kind().try_for_each_edge(|edge| {
                self.charge(1)?;
                omitted_entry |= matches!(
                    edge.role(),
                    SemanticEdgeRoleV1::CallUnwind
                        | SemanticEdgeRoleV1::TailCallUnwind
                        | SemanticEdgeRoleV1::DropUnwind
                        | SemanticEdgeRoleV1::AssertUnwind
                ) && reaching.get(edge.target().index() as usize).copied()
                    == Some(true);
                Ok::<_, ProductionRankedProjectionErrorV1>(())
            })?;
            if omitted_entry {
                return Ok(false);
            }
            for (index, statement) in block.statements().iter().enumerate() {
                self.charge(1)?;
                let site = ScalarAssignmentSiteV1 {
                    block: bb,
                    statement: index,
                };
                match statement.kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        if let SemanticRvalueKindV1::Borrow { place, .. }
                        | SemanticRvalueKindV1::AddressOf { place, .. } =
                            assignment.value().kind()
                            && place.local().index() as usize == source
                            && !same_site(site, borrow)
                        {
                            return Ok(false);
                        }
                        let mut moved = false;
                        assignment.value().kind().try_visit_operands(|operand| {
                            self.charge(1)?;
                            moved |= matches!(operand, SemanticOperandV1::Move(place) if place.local().index() as usize == source);
                            Ok::<_, ProductionRankedProjectionErrorV1>(())
                        })?;
                        if moved {
                            return Ok(false);
                        }
                    }
                    SemanticStatementKindV1::StorageLive(local)
                        if local.index() as usize == source =>
                    {
                        if !self.assignment_dominates_use(
                            site,
                            definition.block,
                            definition.statement,
                        )? {
                            return Ok(false);
                        }
                    }
                    SemanticStatementKindV1::StorageDead(local)
                        if local.index() as usize == source && reaching[bb] =>
                    {
                        return Ok(false);
                    }
                    _ => {}
                }
            }
            let mut moved = false;
            if matches!(block.terminator().kind(), SemanticTerminatorKindV1::Drop { place, .. } if place.local().index() as usize == source)
            {
                return Ok(false);
            }
            visit_terminator_operands(block.terminator().kind(), |operand| {
                self.charge(1)?;
                moved |= matches!(operand, SemanticOperandV1::Move(place) if place.local().index() as usize == source);
                Ok::<_, ProductionRankedProjectionErrorV1>(())
            })?;
            if moved {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn capture_segment_v1(
        &mut self,
        source: usize,
        links: &[Option<Link>],
        borrow: ScalarAssignmentSiteV1,
        read: ScalarAssignmentSiteV1,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        let mut segment = [usize::MAX; MAX_SEGMENT];
        let mut length = 0;
        let mut current = read.block;
        loop {
            self.charge(1)?;
            if length == MAX_SEGMENT
                || segment[..length].contains(&current)
                || !self.graph.is_entry_reachable(current)
            {
                return Ok(false);
            }
            segment[length] = current;
            length += 1;
            if current == borrow.block {
                break;
            }
            let count = self.graph.predecessors(current).map_or(0, <[usize]>::len);
            self.charge(count)?;
            let Some([previous]) = self.graph.predecessors(current) else {
                return Ok(false);
            };
            current = *previous;
        }
        let is_chain = |local: usize| links.iter().flatten().any(|link| link.path.local == local);
        for &bb in &segment[..length] {
            let block = &self.function.blocks()[bb];
            let start = if bb == borrow.block {
                borrow.statement
            } else {
                0
            };
            let end = if bb == read.block {
                read.statement
            } else {
                block.statements().len()
            };
            if start > end || end > block.statements().len() {
                return Ok(false);
            }
            for (offset, statement) in block.statements()[start..end].iter().enumerate() {
                self.charge(links.len() + 1)?;
                let site = ScalarAssignmentSiteV1 {
                    block: bb,
                    statement: start + offset,
                };
                match statement.kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        if matches!(
                            assignment
                                .destination()
                                .projections()
                                .first()
                                .map(|p| p.kind()),
                            Some(SemanticProjectionKindV1::Dereference)
                        ) || matches!(assignment.value().kind(), SemanticRvalueKindV1::Load(_))
                        {
                            return Ok(false);
                        }
                        if local_definition_index(assignment.destination()) == Some(source) {
                            return Ok(false);
                        }
                        if let Some(local) = local_definition_index(assignment.destination())
                            && is_chain(local)
                            && !links.iter().flatten().any(|link| {
                                link.path.local == local && same_site(link.definition, site)
                            })
                        {
                            return Ok(false);
                        }
                        if let SemanticRvalueKindV1::Borrow { place, .. }
                        | SemanticRvalueKindV1::AddressOf { place, .. } =
                            assignment.value().kind()
                            && (is_chain(place.local().index() as usize)
                                || (place.local().index() as usize == source
                                    && !same_site(site, borrow)))
                        {
                            return Ok(false);
                        }
                        let mut invalid = false;
                        let mut uses = [0u8; MAX_LINKS];
                        assignment.value().kind().try_visit_operands(|operand| {
                            self.charge(links.len() + 1)?;
                            if let SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) = operand {
                                for (index, link) in links.iter().flatten().enumerate().filter(|(_, link)| link.path.local == place.local().index() as usize) {
                                    let disjoint = matches!((link.path.field, place.projections().first()), (Some(field), Some(projection)) if matches!(projection.kind(), SemanticProjectionKindV1::Field(other) if other != field));
                                    invalid |= !disjoint && !same_site(link.consumer, site);
                                    if !disjoint { uses[index] = uses[index].saturating_add(1); invalid |= uses[index] > 1; }
                                }
                            }
                            Ok::<_, ProductionRankedProjectionErrorV1>(())
                        })?;
                        if invalid {
                            return Ok(false);
                        }
                    }
                    SemanticStatementKindV1::StorageLive(local)
                    | SemanticStatementKindV1::StorageDead(local)
                        if local.index() as usize != source
                            && !is_chain(local.index() as usize) => {}
                    SemanticStatementKindV1::StorageLive(local) => {
                        let Some(link) = links
                            .iter()
                            .flatten()
                            .find(|link| link.path.local == local.index() as usize)
                        else {
                            return Ok(false);
                        };
                        // Call expansion enters the frame in the caller block.
                        // Its storage must still precede the exact definition.
                        if !self.assignment_dominates_use(
                            site,
                            link.definition.block,
                            link.definition.statement,
                        )? {
                            return Ok(false);
                        }
                    }
                    SemanticStatementKindV1::Nop => {}
                    _ => return Ok(false),
                }
            }
            if bb == read.block {
                continue;
            }
            let allowed = match block.terminator().kind() {
                SemanticTerminatorKindV1::Goto(_)
                | SemanticTerminatorKindV1::Assert {
                    unwind: SemanticUnwindActionV1::Unreachable,
                    ..
                } => true,
                SemanticTerminatorKindV1::Call(call) => {
                    call.unwind() == SemanticUnwindActionV1::Unreachable
                        && call
                            .destination()
                            .is_some_and(|destination| destination.place().projections().is_empty())
                }
                _ => false,
            };
            if !allowed {
                return Ok(false);
            }
            let mut invalid = false;
            visit_terminator_operands(block.terminator().kind(), |operand| {
                self.charge(links.len() + 1)?;
                invalid |= matches!(operand, SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) if is_chain(place.local().index() as usize));
                Ok::<_, ProductionRankedProjectionErrorV1>(())
            })?;
            if invalid {
                return Ok(false);
            }
        }
        let mut incoming = [0u8; MAX_SEGMENT];
        for (from, block) in self.function.blocks().iter().enumerate() {
            self.charge(1)?;
            let mut invalid = false;
            block.terminator().kind().try_for_each_edge(|edge| {
                self.charge(length)?;
                if let Some(index) = segment[..length - 1]
                    .iter()
                    .position(|&to| to == edge.target().index() as usize)
                {
                    incoming[index] = incoming[index].saturating_add(1);
                    invalid |= incoming[index] != 1 || from != segment[index + 1];
                }
                Ok::<_, ProductionRankedProjectionErrorV1>(())
            })?;
            if invalid {
                return Ok(false);
            }
        }
        Ok(incoming[..length - 1].iter().all(|count| *count == 1))
    }
}

fn visit_terminator_operands<E>(
    terminal: &SemanticTerminatorKindV1,
    mut visit: impl FnMut(&SemanticOperandV1) -> Result<(), E>,
) -> Result<(), E> {
    match terminal {
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => visit(discriminant)?,
        SemanticTerminatorKindV1::Call(call) => {
            for operand in call.arguments() {
                visit(operand)?;
            }
        }
        SemanticTerminatorKindV1::TailCall(call) => {
            for operand in call.arguments() {
                visit(operand)?;
            }
        }
        SemanticTerminatorKindV1::Drop { .. } => {}
        SemanticTerminatorKindV1::Assert {
            condition, message, ..
        } => {
            visit(condition)?;
            match message {
                SemanticAssertMessageV1::BoundsCheck {
                    length: left,
                    index: right,
                }
                | SemanticAssertMessageV1::Overflow { left, right, .. }
                | SemanticAssertMessageV1::MisalignedPointerDereference {
                    required_alignment: left,
                    found_alignment: right,
                } => {
                    visit(left)?;
                    visit(right)?;
                }
                SemanticAssertMessageV1::DivisionByZero(operand)
                | SemanticAssertMessageV1::RemainderByZero(operand) => visit(operand)?,
                SemanticAssertMessageV1::NullPointerDereference
                | SemanticAssertMessageV1::ResumedAfterReturn
                | SemanticAssertMessageV1::ResumedAfterPanic => {}
            }
        }
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::FalseEdge { .. }
        | SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => {}
    }
    Ok(())
}

#[cfg(test)]
#[path = "shared_scalar_range_source_v1/tests.rs"]
mod tests;
