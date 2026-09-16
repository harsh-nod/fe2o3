// A closed source-use grammar proves absence of writes, not just absence of
// lexically named write methods. Unknown operations cannot erase a root alias.
fn audit_consumed_read_only_roots_v1(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    allocations: &[Option<AllocationContractV1>],
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let mut work = 0;
    let mut roots = vec![None; function.abi().source_input_types().len()];
    let mut conversions = Vec::new();
    for (block_index, block) in function.blocks().iter().enumerate() {
        charge_capability_dataflow_work_v1(&mut work, 1)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::DisjointSliceIntoReadOnly {
                    slice,
                    view,
                    element,
                },
            ..
        }) = callables.get(call.callee().index() as usize)
        else {
            continue;
        };
        let Some(origin) =
            consumed_read_only_origin_v1(call, function, 0, *view, *element, allocations)
        else {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a consuming readonly conversion lacks exact exclusive allocation custody",
            ));
        };
        let ProjectedReadValueV1::AllocationExtent(argument) = origin.columns else {
            unreachable!("consumed view retains its actual argument extent")
        };
        if function.abi().source_input_types().get(argument as usize) != Some(slice)
            || function
                .abi()
                .source_argument_ownership()
                .get(argument as usize)
                != Some(&SemanticSourceArgumentOwnershipV1::ExclusiveOwner)
            || call.arguments()[0].ty() != *slice
            || !matches!(
                types
                    .get(element.index() as usize)
                    .map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Scalar(
                    SemanticScalarTypeV1::Integer {
                        bits: 16,
                        signed: false
                    } | SemanticScalarTypeV1::Float { bits: 32 }
                ))
            )
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a consumed readonly root changed ownership, slice, or supported element identity",
            ));
        }
        let root = ConsumedReadOnlyRootV1 {
            argument,
            slice: *slice,
            view: *view,
            element: *element,
            conversion_block: block_index,
        };
        let slot = roots.get_mut(argument as usize).ok_or(
            ProductionRankedProjectionErrorV1::Incomplete("readonly root argument is absent"),
        )?;
        if slot.replace(root).is_some() {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "one readonly allocation has multiple consuming conversions",
            ));
        }
        conversions.push((simple_call_destination(call)?.index() as usize, argument));
    }
    if conversions.is_empty() {
        return Ok(());
    }
    if allocations.len() != function.locals().len() {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "readonly allocation custody has the wrong local extent",
        ));
    }
    charge_capability_dataflow_work_v1(&mut work, function.locals().len())?;
    let mut aliases = vec![None; function.locals().len()];
    for (local, allocation) in allocations.iter().enumerate() {
        if let Some(argument) = allocation.and_then(|allocation| {
            allocation
                .allocation_origin
                .checked_sub(1)
                .and_then(|value| u32::try_from(value).ok())
        }) && roots.get(argument as usize).is_some_and(Option::is_some)
        {
            aliases[local] = Some(argument);
        }
    }
    for (local, argument) in conversions {
        if aliases.get_mut(local).is_none_or(|slot| {
            slot.replace(argument)
                .is_some_and(|previous| previous != argument)
        }) {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a readonly conversion destination aliases another root",
            ));
        }
    }
    // Only transparent whole-local transport can preserve custody. Any other
    // source use is rejected in the exhaustive pass, even if its result is dead.
    let mut edges = vec![Vec::new(); aliases.len()];
    let mut edge_count = 0;
    for block in function.blocks() {
        for statement in block.statements() {
            charge_capability_dataflow_work_v1(&mut work, 1)?;
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty() {
                continue;
            }
            let source = match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => simple_operand_local(operand),
                SemanticRvalueKindV1::Borrow { place, .. } if place.projections().is_empty() => {
                    Some(place.local())
                }
                _ => None,
            };
            if let Some(source) = source {
                push_local_provenance_edge_v1(
                    &mut edges,
                    source.index() as usize,
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
    while let Some(source) = pending.pop_front() {
        for &destination in &edges[source] {
            charge_capability_dataflow_work_v1(&mut work, 1)?;
            match (aliases[source], aliases[destination]) {
                (Some(root), None) => {
                    aliases[destination] = Some(root);
                    pending.push_back(destination);
                }
                (Some(root), Some(other)) if root != other => {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a readonly local may alias distinct allocation roots",
                    ));
                }
                _ => {}
            }
        }
    }
    for (local, root) in function.locals().iter().zip(&aliases) {
        charge_capability_dataflow_work_v1(&mut work, 1)?;
        if let Some(root) = root {
            let root =
                roots[*root as usize].ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                    "a readonly alias has no authenticated source root",
                ))?;
            if local.role() == SemanticLocalRoleV1::Return
                || !consumed_read_only_alias_type_v1(types, local.ty(), root)
            {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a readonly root escapes through a return, raw pointer, or unsupported alias type",
                ));
            }
        }
    }
    let after_conversion = consumed_read_only_successor_custody_v1(function, &roots, &mut work)?;
    let mut audit = ConsumedReadOnlyAuditV1 {
        types,
        roots: &roots,
        aliases: &aliases,
        after_conversion: &after_conversion,
        block: 0,
        work,
    };
    for (block_index, block) in function.blocks().iter().enumerate() {
        audit.block = block_index;
        for statement in block.statements() {
            audit.statement(statement.kind())?;
        }
        audit.terminator(callables, block_index, block.terminator().kind())?;
    }
    Ok(())
}

struct ConsumedReadOnlyAuditV1<'a> {
    types: &'a [SemanticTypeDeclV1],
    roots: &'a [Option<ConsumedReadOnlyRootV1>],
    aliases: &'a [Option<u32>],
    after_conversion: &'a [Option<Vec<bool>>],
    block: usize,
    work: usize,
}

impl ConsumedReadOnlyAuditV1<'_> {
    fn reject() -> ProductionRankedProjectionErrorV1 {
        ProductionRankedProjectionErrorV1::Incomplete(
            "a consumed readonly allocation is written, escaped, or used outside the closed read grammar",
        )
    }

    fn root(
        &mut self,
        place: &SemanticPlaceV1,
    ) -> Result<Option<ConsumedReadOnlyRootV1>, ProductionRankedProjectionErrorV1> {
        charge_capability_dataflow_work_v1(&mut self.work, place.projections().len() + 1)?;
        for projection in place.projections() {
            if let SemanticProjectionKindV1::Index(index) = projection.kind()
                && self
                    .aliases
                    .get(index.index() as usize)
                    .is_some_and(Option::is_some)
            {
                return Err(Self::reject());
            }
        }
        let root = self
            .aliases
            .get(place.local().index() as usize)
            .copied()
            .flatten()
            .and_then(|root| self.roots.get(root as usize).copied().flatten());
        if let Some(root) = root
            && self.after_conversion[root.argument as usize]
                .as_ref()
                .is_some_and(|blocks| blocks[self.block])
            && (place.ty() == root.slice
                || consumed_read_only_shared_reference_v1(self.types, place.ty(), root.slice))
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a consumed readonly source allocation or alias is reused after conversion",
            ));
        }
        Ok(root)
    }

    fn operand_root(
        &mut self,
        operand: &SemanticOperandV1,
    ) -> Result<Option<ConsumedReadOnlyRootV1>, ProductionRankedProjectionErrorV1> {
        charge_capability_dataflow_work_v1(&mut self.work, 1)?;
        raw_operand_place(operand).map_or(Ok(None), |place| self.root(place))
    }

    fn no_place(
        &mut self,
        place: &SemanticPlaceV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        if self.root(place)?.is_some() {
            Err(Self::reject())
        } else {
            Ok(())
        }
    }

    fn no_operand(
        &mut self,
        operand: &SemanticOperandV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        if self.operand_root(operand)?.is_some() {
            Err(Self::reject())
        } else {
            Ok(())
        }
    }

    fn statement(
        &mut self,
        statement: &SemanticStatementKindV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        charge_capability_dataflow_work_v1(&mut self.work, 1)?;
        match statement {
            SemanticStatementKindV1::Assign(assignment) => {
                let destination = assignment.destination();
                if let Some(root) = self.root(destination)? {
                    if !destination.projections().is_empty() {
                        return Err(Self::reject());
                    }
                    let source = match assignment.value().kind() {
                        SemanticRvalueKindV1::Use(operand) => {
                            let place = raw_operand_place(operand).ok_or_else(Self::reject)?;
                            let copy_allowed = !matches!(operand, SemanticOperandV1::Copy(_))
                                || consumed_read_only_shared_reference_v1(
                                    self.types,
                                    place.ty(),
                                    root.slice,
                                )
                                || consumed_read_only_shared_reference_v1(
                                    self.types,
                                    place.ty(),
                                    root.view,
                                );
                            if !copy_allowed || destination.ty() != place.ty() {
                                return Err(Self::reject());
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
                        _ => return Err(Self::reject()),
                    };
                    if !source.projections().is_empty() || self.root(source)? != Some(root) {
                        return Err(Self::reject());
                    }
                } else {
                    assignment
                        .value()
                        .kind()
                        .try_visit_operands(|operand| self.no_operand(operand))?;
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
        block: usize,
        terminator: &SemanticTerminatorKindV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        charge_capability_dataflow_work_v1(&mut self.work, 1)?;
        match terminator {
            SemanticTerminatorKindV1::Call(call) => self.call(callables, block, call)?,
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

    fn call(
        &mut self,
        callables: &[SemanticCallableDeclV1],
        block: usize,
        call: &SemanticDirectCallV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        let receiver_root = call
            .arguments()
            .first()
            .map(|operand| self.operand_root(operand))
            .transpose()?
            .flatten();
        let operation = match callables.get(call.callee().index() as usize) {
            Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) => Some(operation),
            _ => None,
        };
        let Some(root) = receiver_root else {
            for argument in call.arguments() {
                self.no_operand(argument)?;
            }
            if let Some(destination) = call.destination() {
                self.no_place(destination.place())?;
            }
            if matches!(
                operation,
                Some(
                    SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLen { .. }
                        | SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLoadOr { .. }
                )
            ) {
                return Err(Self::reject());
            }
            return Ok(());
        };
        let receiver = raw_operand_place(&call.arguments()[0]).ok_or_else(Self::reject)?;
        let destination = call.destination().ok_or_else(Self::reject)?.place();
        if !receiver.projections().is_empty()
            || !destination.projections().is_empty()
            || matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
        {
            return Err(Self::reject());
        }
        let approved = match operation {
            Some(SemanticCompilerIntrinsicOperationV1::DisjointSliceIntoReadOnly {
                slice,
                view,
                element,
            }) => {
                call.arguments().len() == 1
                    && block == root.conversion_block
                    && (*slice, *view, *element) == (root.slice, root.view, root.element)
                    && receiver.ty() == root.slice
                    && destination.ty() == root.view
                    && matches!(
                        call.arguments()[0],
                        SemanticOperandV1::Move(_) | SemanticOperandV1::Copy(_)
                    )
                    && self.root(destination)? == Some(root)
            }
            Some(SemanticCompilerIntrinsicOperationV1::DisjointSliceLen {
                disjoint_slice, ..
            }) => {
                call.arguments().len() == 1
                    && *disjoint_slice == root.slice
                    && consumed_read_only_shared_reference_v1(self.types, receiver.ty(), root.slice)
                    && unsigned_index_bits_v1(self.types, destination.ty()) == Some(64)
                    && self.root(destination)?.is_none()
            }
            Some(SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLen { view }) => {
                call.arguments().len() == 1
                    && *view == root.view
                    && consumed_read_only_shared_reference_v1(self.types, receiver.ty(), root.view)
                    && unsigned_index_bits_v1(self.types, destination.ty()) == Some(64)
                    && self.root(destination)?.is_none()
            }
            Some(SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLoadOr {
                view,
                element,
            }) => {
                for argument in &call.arguments()[1..] {
                    self.no_operand(argument)?;
                }
                call.arguments().len() == 3
                    && (*view, *element) == (root.view, root.element)
                    && consumed_read_only_shared_reference_v1(self.types, receiver.ty(), root.view)
                    && unsigned_index_bits_v1(self.types, call.arguments()[1].ty()) == Some(64)
                    && call.arguments()[2].ty() == root.element
                    && destination.ty() == root.element
                    && self.root(destination)?.is_none()
            }
            _ => false,
        };
        if approved {
            Ok(())
        } else {
            Err(Self::reject())
        }
    }
}
