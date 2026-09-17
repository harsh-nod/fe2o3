fn static_publication_audit_uses_v1(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    allocations: &[Option<AllocationContractV1>],
    source: &StaticPublicationSourceV1,
    work: &mut usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    charge_capability_dataflow_work_v1(work, function.locals().len())?;
    let mut aliases = vec![None; function.locals().len()];
    for (local, allocation) in allocations.iter().enumerate() {
        charge_capability_dataflow_work_v1(work, 1)?;
        for (root, expected) in [source.payload, source.flags].iter().enumerate() {
            if allocation.is_some_and(|allocation| {
                allocation.allocation_origin == expected.allocation.allocation_origin
            }) {
                aliases[local] = Some(root);
            }
        }
    }
    let mut edges = vec![Vec::new(); aliases.len()];
    let mut edge_count = 0;
    for block in function.blocks() {
        for statement in block.statements() {
            charge_capability_dataflow_work_v1(work, 1)?;
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty() {
                continue;
            }
            let origin = match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => simple_operand_local(operand),
                SemanticRvalueKindV1::Borrow { place, .. } if place.projections().is_empty() => {
                    Some(place.local())
                }
                _ => None,
            };
            if let Some(origin) = origin {
                push_local_provenance_edge_v1(
                    &mut edges,
                    origin.index() as usize,
                    assignment.destination().local().index() as usize,
                    &mut edge_count,
                )?;
            }
        }
    }
    let mut pending = VecDeque::new();
    for (local, root) in aliases.iter().enumerate() {
        if root.is_some() {
            pending.push_back(local);
        }
    }
    while let Some(local) = pending.pop_front() {
        for &next in &edges[local] {
            charge_capability_dataflow_work_v1(work, 1)?;
            match (aliases[local], aliases[next]) {
                (Some(root), None) => {
                    aliases[next] = Some(root);
                    pending.push_back(next);
                }
                (Some(root), Some(other)) if root != other => {
                    return Err(static_publication_reject_v1());
                }
                _ => {}
            }
        }
    }
    for (local, root) in function.locals().iter().zip(&aliases) {
        charge_capability_dataflow_work_v1(work, 1)?;
        if let Some(root) = root {
            let expected = [source.payload, source.flags][*root];
            if local.role() == SemanticLocalRoleV1::Return
                || (local.ty() != expected.ty
                    && !consumed_read_only_shared_reference_v1(types, local.ty(), expected.ty))
            {
                return Err(static_publication_reject_v1());
            }
        }
    }
    // Both branches consume the same custody. A successor reaching either site
    // would permit a second attempt; every later source-root use is also closed.
    charge_capability_dataflow_work_v1(work, function.blocks().len())?;
    let mut consumed = vec![false; function.blocks().len()];
    let mut queue = VecDeque::from([source.producer.block, source.consumer.block]);
    while let Some(block) = queue.pop_front() {
        charge_capability_dataflow_work_v1(work, 1)?;
        let body = function
            .blocks()
            .get(block)
            .ok_or_else(static_publication_reject_v1)?;
        body.terminator()
            .kind()
            .try_for_each_edge::<ProductionRankedProjectionErrorV1>(|edge| {
                charge_capability_dataflow_work_v1(work, 1)?;
                let target = edge.target().index() as usize;
                if target == source.producer.block || target == source.consumer.block {
                    return Err(static_publication_reject_v1());
                }
                let reached = consumed
                    .get_mut(target)
                    .ok_or_else(static_publication_reject_v1)?;
                if !*reached {
                    *reached = true;
                    queue.push_back(target);
                }
                Ok(())
            })?;
    }
    let mut audit = StaticPublicationUseAuditV1 {
        types,
        source,
        aliases: &aliases,
        consumed: &consumed,
        block: 0,
        statement_index: 0,
        work,
    };
    for (block, body) in function.blocks().iter().enumerate() {
        audit.block = block;
        for (index, statement) in body.statements().iter().enumerate() {
            audit.statement_index = index;
            audit.statement(statement.kind())?;
        }
        audit.terminator(callables, body.terminator().kind())?;
    }
    Ok(())
}

struct StaticPublicationUseAuditV1<'a> {
    types: &'a [SemanticTypeDeclV1],
    source: &'a StaticPublicationSourceV1,
    aliases: &'a [Option<usize>],
    consumed: &'a [bool],
    block: usize,
    statement_index: usize,
    work: &'a mut usize,
}

impl StaticPublicationUseAuditV1<'_> {
    fn root(
        &mut self,
        place: &SemanticPlaceV1,
    ) -> Result<Option<usize>, ProductionRankedProjectionErrorV1> {
        charge_capability_dataflow_work_v1(self.work, place.projections().len() + 1)?;
        for projection in place.projections() {
            if let SemanticProjectionKindV1::Index(index) = projection.kind()
                && self
                    .aliases
                    .get(index.index() as usize)
                    .is_some_and(Option::is_some)
            {
                return Err(static_publication_reject_v1());
            }
        }
        let root = self
            .aliases
            .get(place.local().index() as usize)
            .copied()
            .flatten();
        if root.is_some() && self.consumed[self.block] {
            return Err(static_publication_reject_v1());
        }
        Ok(root)
    }

    fn no_place(
        &mut self,
        place: &SemanticPlaceV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        if self.root(place)?.is_some() {
            Err(static_publication_reject_v1())
        } else {
            Ok(())
        }
    }

    fn no_operand(
        &mut self,
        operand: &SemanticOperandV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        charge_capability_dataflow_work_v1(self.work, 1)?;
        raw_operand_place(operand).map_or(Ok(()), |place| self.no_place(place))
    }

    fn statement(
        &mut self,
        statement: &SemanticStatementKindV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        charge_capability_dataflow_work_v1(self.work, 1)?;
        match statement {
            SemanticStatementKindV1::Assign(assignment) => {
                let destination = assignment.destination();
                if let Some(root) = self.root(destination)? {
                    let expected = [self.source.payload, self.source.flags][root];
                    if !destination.projections().is_empty() {
                        return Err(static_publication_reject_v1());
                    }
                    let origin = match assignment.value().kind() {
                        SemanticRvalueKindV1::Use(operand) => {
                            let place = raw_operand_place(operand)
                                .ok_or_else(static_publication_reject_v1)?;
                            let metadata_copy =
                                self.source.metadata_snapshot.is_some_and(|snapshot| {
                                    snapshot.block == self.block
                                        && snapshot.statement == self.statement_index
                                        && snapshot.local == destination.local()
                                });
                            if destination.ty() != place.ty()
                                || (root == 0
                                    && !consumed_read_only_shared_reference_v1(
                                        self.types,
                                        place.ty(),
                                        expected.ty,
                                    )
                                    && !metadata_copy)
                            {
                                return Err(static_publication_reject_v1());
                            }
                            place
                        }
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place,
                        } if consumed_read_only_shared_reference_v1(
                            self.types,
                            destination.ty(),
                            place.ty(),
                        ) =>
                        {
                            place
                        }
                        _ => return Err(static_publication_reject_v1()),
                    };
                    if !origin.projections().is_empty() || self.root(origin)? != Some(root) {
                        return Err(static_publication_reject_v1());
                    }
                } else {
                    // Slice extent reads only the original fat-pointer metadata.
                    let flag_metadata = match assignment.value().kind() {
                        SemanticRvalueKindV1::Unary {
                            operation: SemanticUnaryOpV1::PointerMetadata,
                            operand: SemanticOperandV1::Copy(place),
                        } if place.local() == self.source.flags.local
                            && place.ty() == self.source.flags.ty
                            && place.projections().is_empty()
                            && destination.ty() == assignment.value().result_type()
                            && unsigned_index_bits_v1(self.types, destination.ty()) == Some(64) =>
                        {
                            self.root(place)? == Some(1)
                        }
                        _ => false,
                    };
                    if !flag_metadata {
                        assignment
                            .value()
                            .kind()
                            .try_visit_operands(|operand| self.no_operand(operand))?;
                    }
                    match assignment.value().kind() {
                        SemanticRvalueKindV1::Load(load) => self.no_place(load.source())?,
                        SemanticRvalueKindV1::Borrow { place, .. }
                        | SemanticRvalueKindV1::AddressOf { place, .. }
                        | SemanticRvalueKindV1::Length(place)
                        | SemanticRvalueKindV1::Discriminant(place) => self.no_place(place)?,
                        SemanticRvalueKindV1::Use(_)
                        | SemanticRvalueKindV1::Unary { .. }
                        | SemanticRvalueKindV1::Binary { .. }
                        | SemanticRvalueKindV1::CheckedBinary { .. }
                        | SemanticRvalueKindV1::UncheckedBinary { .. }
                        | SemanticRvalueKindV1::Cast { .. }
                        | SemanticRvalueKindV1::Aggregate { .. } => {}
                    }
                }
            }
            SemanticStatementKindV1::Store(store) => {
                self.no_place(store.destination())?;
                self.no_operand(store.value())?;
            }
            SemanticStatementKindV1::AtomicRmw(atomic) => {
                self.no_place(atomic.address())?;
                self.no_place(atomic.destination())?;
                self.no_operand(atomic.value())?;
            }
            SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                self.no_place(atomic.address())?;
                self.no_place(atomic.destination())?;
                self.no_operand(atomic.expected())?;
                self.no_operand(atomic.replacement())?;
            }
            SemanticStatementKindV1::SetDiscriminant { place, .. }
            | SemanticStatementKindV1::Deinitialize(place) => self.no_place(place)?,
            SemanticStatementKindV1::Assume(operand) => self.no_operand(operand)?,
            SemanticStatementKindV1::StorageLive(_)
            | SemanticStatementKindV1::StorageDead(_)
            | SemanticStatementKindV1::Nop => {}
        }
        Ok(())
    }

    fn terminator(
        &mut self,
        callables: &[SemanticCallableDeclV1],
        terminator: &SemanticTerminatorKindV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        charge_capability_dataflow_work_v1(self.work, 1)?;
        match terminator {
            SemanticTerminatorKindV1::Call(call) => {
                if static_publication_intrinsic_v1(callables, call).is_some() {
                    if ![self.source.producer.block, self.source.consumer.block]
                        .contains(&self.block)
                    {
                        return Err(static_publication_reject_v1());
                    }
                    for (index, argument) in call.arguments().iter().enumerate() {
                        if index < 2 {
                            let place = raw_operand_place(argument)
                                .ok_or_else(static_publication_reject_v1)?;
                            if self.root(place)? != Some(index) {
                                return Err(static_publication_reject_v1());
                            }
                        } else {
                            self.no_operand(argument)?;
                        }
                    }
                } else if let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation:
                        SemanticCompilerIntrinsicOperationV1::DisjointSliceLen {
                            disjoint_slice, ..
                        },
                    ..
                }) = callables.get(call.callee().index() as usize)
                    && call.arguments().len() == 1
                    && *disjoint_slice == self.source.payload.ty
                    && raw_operand_place(&call.arguments()[0]).is_some_and(|place| {
                        place.projections().is_empty()
                            && self.aliases.get(place.local().index() as usize) == Some(&Some(0))
                            && consumed_read_only_shared_reference_v1(
                                self.types,
                                place.ty(),
                                self.source.payload.ty,
                            )
                    })
                {
                    let place = raw_operand_place(&call.arguments()[0])
                        .ok_or_else(static_publication_reject_v1)?;
                    if self.root(place)? != Some(0) {
                        return Err(static_publication_reject_v1());
                    }
                } else {
                    for argument in call.arguments() {
                        self.no_operand(argument)?;
                    }
                }
                if let Some(destination) = call.destination() {
                    self.no_place(destination.place())?;
                }
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                for argument in call.arguments() {
                    self.no_operand(argument)?;
                }
            }
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                self.no_operand(discriminant)?
            }
            SemanticTerminatorKindV1::Drop { place, .. } => self.no_place(place)?,
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => {
                self.no_operand(condition)?;
                match message {
                    SemanticAssertMessageV1::BoundsCheck { length, index } => {
                        self.no_operand(length)?;
                        self.no_operand(index)?;
                    }
                    SemanticAssertMessageV1::Overflow { left, right, .. } => {
                        self.no_operand(left)?;
                        self.no_operand(right)?;
                    }
                    SemanticAssertMessageV1::DivisionByZero(operand)
                    | SemanticAssertMessageV1::RemainderByZero(operand) => {
                        self.no_operand(operand)?
                    }
                    SemanticAssertMessageV1::MisalignedPointerDereference {
                        required_alignment,
                        found_alignment,
                    } => {
                        self.no_operand(required_alignment)?;
                        self.no_operand(found_alignment)?;
                    }
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
}
