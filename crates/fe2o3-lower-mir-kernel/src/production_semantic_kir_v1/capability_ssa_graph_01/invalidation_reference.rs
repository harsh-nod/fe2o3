fn reference_moves(operand: &SemanticOperandV1, local: u32) -> bool {
    matches!(operand, SemanticOperandV1::Move(place) if place.local().index() == local)
}

fn reference_invalidates(statement: &SemanticStatementKindV1, local: u32) -> bool {
    match statement {
        SemanticStatementKindV1::StorageLive(id) | SemanticStatementKindV1::StorageDead(id) => {
            id.index() == local
        }
        SemanticStatementKindV1::Deinitialize(place)
        | SemanticStatementKindV1::SetDiscriminant { place, .. } => place.local().index() == local,
        SemanticStatementKindV1::Assign(assignment) => {
            if assignment.destination().local().index() == local {
                return true;
            }
            match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand)
                | SemanticRvalueKindV1::Unary { operand, .. }
                | SemanticRvalueKindV1::Cast { operand, .. } => reference_moves(operand, local),
                SemanticRvalueKindV1::Binary { left, right, .. } => {
                    reference_moves(left, local) || reference_moves(right, local)
                }
                SemanticRvalueKindV1::CheckedBinary(binary) => {
                    reference_moves(binary.left(), local) || reference_moves(binary.right(), local)
                }
                SemanticRvalueKindV1::UncheckedBinary(binary) => {
                    reference_moves(binary.left(), local) || reference_moves(binary.right(), local)
                }
                SemanticRvalueKindV1::Aggregate(aggregate) => aggregate
                    .operands()
                    .iter()
                    .any(|operand| reference_moves(operand, local)),
                SemanticRvalueKindV1::Borrow { kind, place } => {
                    place.local().index() == local && *kind != SemanticBorrowKindV1::Shared
                }
                SemanticRvalueKindV1::AddressOf { place, .. } => place.local().index() == local,
                SemanticRvalueKindV1::Length(_)
                | SemanticRvalueKindV1::Discriminant(_)
                | SemanticRvalueKindV1::Load(_) => false,
            }
        }
        // These operations are not valid ways to mutate a tracked capability.
        SemanticStatementKindV1::Store(store) => {
            store.destination().local().index() == local || reference_moves(store.value(), local)
        }
        SemanticStatementKindV1::AtomicRmw(atomic) => {
            atomic.address().local().index() == local
                || atomic.destination().local().index() == local
                || reference_moves(atomic.value(), local)
        }
        SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
            atomic.address().local().index() == local
                || atomic.destination().local().index() == local
                || reference_moves(atomic.expected(), local)
                || reference_moves(atomic.replacement(), local)
        }
        SemanticStatementKindV1::Assume(operand) => reference_moves(operand, local),
        SemanticStatementKindV1::Nop => false,
    }
}
