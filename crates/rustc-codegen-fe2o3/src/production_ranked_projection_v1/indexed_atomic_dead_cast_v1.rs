// Inlining AtomicU32 can retain an unused UnsafeCell-to-Align4 sibling cast.
// This exception proves it dead; it never adds pointer or allocation authority.
fn indexed_atomic_dead_cast_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    inventory: &AuthenticatedAtomicAllocationsV1,
    site: ScalarAssignmentSiteV1,
    assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
) -> Result<bool, ProductionRankedProjectionErrorV1> {
    let SemanticRvalueKindV1::Cast {
        kind: SemanticCastKindV1::Pointer,
        operand,
    } = assignment.value().kind()
    else {
        return Ok(false);
    };
    let destination = assignment.destination();
    let Some(declaration) = function.locals().get(destination.local().index() as usize) else {
        return Ok(false);
    };
    if !destination.projections().is_empty()
        || declaration.role() != SemanticLocalRoleV1::Temporary
        || declaration.ty() != destination.ty()
        || destination.ty() != assignment.value().result_type()
    {
        return Ok(false);
    }
    let Some(source_local) = simple_operand_local(operand) else {
        return Ok(false);
    };
    if source_local == destination.local() || !inventory.contains_local(source_local)? {
        return Ok(false);
    }
    let Some(source) = atomic_u32_pointer_v1(types, operand.ty()) else {
        return Ok(false);
    };
    let Some(target) = atomic_u32_pointer_v1(types, destination.ty()) else {
        return Ok(false);
    };
    if source.kind() != SemanticPointerKindV1::Raw || target.kind() != SemanticPointerKindV1::Raw {
        return Ok(false);
    }
    let Some(storage) = types.get(source.pointee().index() as usize) else {
        return Ok(false);
    };
    let Some(field) = types.get(target.pointee().index() as usize) else {
        return Ok(false);
    };
    let SemanticTypeShapeV1::Aggregate(fields) = storage.shape() else {
        return Ok(false);
    };
    let fe2o3_mir_model::semantic_mir_v1::SemanticFieldsShapeV1::Arbitrary {
        source_order_offsets_bytes,
        ..
    } = storage.layout().fields()
    else {
        return Ok(false);
    };
    if storage.layout().size_bytes() != Some(4)
        || storage.layout().alignment_bytes() != 4
        || field.layout().size_bytes() != Some(4)
        || field.layout().alignment_bytes() != 4
        || fields.fields() != [target.pointee()]
        || source_order_offsets_bytes.as_ref() != [0]
    {
        return Ok(false);
    }
    indexed_atomic_temporary_unused_v1(function, inventory, site, destination.local())
}

fn indexed_atomic_temporary_unused_v1(
    function: &SemanticFunctionDeclV1,
    inventory: &AuthenticatedAtomicAllocationsV1,
    definition: ScalarAssignmentSiteV1,
    local: SemanticLocalIdV1,
) -> Result<bool, ProductionRankedProjectionErrorV1> {
    // Return implicitly reads its return local, so only a temporary can qualify.
    if function
        .locals()
        .get(local.index() as usize)
        .is_none_or(|decl| decl.role() != SemanticLocalRoleV1::Temporary)
    {
        return Ok(false);
    }
    let place_mentions =
        |place: &SemanticPlaceV1| -> Result<bool, ProductionRankedProjectionErrorV1> {
            inventory.charge(place.projections().len() + 1)?;
            Ok(place.local() == local || place.projections().iter().any(|projection| {
            matches!(projection.kind(), SemanticProjectionKindV1::Index(index) if index == local)
        }))
        };
    let operand_mentions =
        |operand: &SemanticOperandV1| -> Result<bool, ProductionRankedProjectionErrorV1> {
            inventory.charge(1)?;
            raw_operand_place(operand).map_or(Ok(false), &place_mentions)
        };
    for (block, body) in function.blocks().iter().enumerate() {
        for (statement, record) in body.statements().iter().enumerate() {
            inventory.charge(1)?;
            if block == definition.block && statement == definition.statement {
                continue;
            }
            let mentions = match record.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    let mut operands = false;
                    assignment.value().kind().try_visit_operands(|operand| {
                        operands |= operand_mentions(operand)?;
                        Ok::<(), ProductionRankedProjectionErrorV1>(())
                    })?;
                    place_mentions(assignment.destination())?
                        || operands
                        || match assignment.value().kind() {
                            SemanticRvalueKindV1::Load(load) => place_mentions(load.source())?,
                            SemanticRvalueKindV1::Borrow { place, .. }
                            | SemanticRvalueKindV1::AddressOf { place, .. }
                            | SemanticRvalueKindV1::Length(place)
                            | SemanticRvalueKindV1::Discriminant(place) => place_mentions(place)?,
                            SemanticRvalueKindV1::Use(_)
                            | SemanticRvalueKindV1::Unary { .. }
                            | SemanticRvalueKindV1::Binary { .. }
                            | SemanticRvalueKindV1::CheckedBinary { .. }
                            | SemanticRvalueKindV1::UncheckedBinary { .. }
                            | SemanticRvalueKindV1::Cast { .. }
                            | SemanticRvalueKindV1::Aggregate { .. } => false,
                        }
                }
                SemanticStatementKindV1::Store(store) => {
                    place_mentions(store.destination())? || operand_mentions(store.value())?
                }
                SemanticStatementKindV1::AtomicRmw(atomic) => {
                    place_mentions(atomic.destination())?
                        || place_mentions(atomic.address())?
                        || operand_mentions(atomic.value())?
                }
                SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                    place_mentions(atomic.destination())?
                        || place_mentions(atomic.address())?
                        || operand_mentions(atomic.expected())?
                        || operand_mentions(atomic.replacement())?
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => place_mentions(place)?,
                SemanticStatementKindV1::Assume(operand) => operand_mentions(operand)?,
                SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Nop => false,
            };
            if mentions {
                return Ok(false);
            }
        }
        inventory.charge(1)?;
        let mentions = match body.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => {
                let mut mentions = false;
                for argument in call.arguments() {
                    mentions |= operand_mentions(argument)?;
                }
                if let Some(destination) = call.destination() {
                    mentions |= place_mentions(destination.place())?;
                }
                mentions
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                let mut mentions = false;
                for argument in call.arguments() {
                    mentions |= operand_mentions(argument)?;
                }
                mentions
            }
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                operand_mentions(discriminant)?
            }
            SemanticTerminatorKindV1::Drop { place, .. } => place_mentions(place)?,
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => {
                operand_mentions(condition)?
                    || match message {
                        SemanticAssertMessageV1::BoundsCheck { length, index } => {
                            operand_mentions(length)? || operand_mentions(index)?
                        }
                        SemanticAssertMessageV1::Overflow { left, right, .. } => {
                            operand_mentions(left)? || operand_mentions(right)?
                        }
                        SemanticAssertMessageV1::DivisionByZero(operand)
                        | SemanticAssertMessageV1::RemainderByZero(operand) => {
                            operand_mentions(operand)?
                        }
                        SemanticAssertMessageV1::MisalignedPointerDereference {
                            required_alignment,
                            found_alignment,
                        } => {
                            operand_mentions(required_alignment)?
                                || operand_mentions(found_alignment)?
                        }
                        SemanticAssertMessageV1::NullPointerDereference
                        | SemanticAssertMessageV1::ResumedAfterReturn
                        | SemanticAssertMessageV1::ResumedAfterPanic => false,
                    }
            }
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => false,
        };
        if mentions {
            return Ok(false);
        }
    }
    Ok(true)
}
