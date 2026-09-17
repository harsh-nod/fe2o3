#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StaticPublicationMetadataSnapshotV1 {
    local: SemanticLocalIdV1,
    block: usize,
    statement: usize,
}

// Force-inlining retains an owning-typed metadata snapshot although the two
// terminals still consume the original argument. This is not owning transport.
fn static_publication_metadata_snapshot_v1(
    proofs: &mut SemanticAssertProofsV1<'_>,
    callables: &[SemanticCallableDeclV1],
    payload: StaticPublicationRootV1,
) -> Result<Option<StaticPublicationMetadataSnapshotV1>, ProductionRankedProjectionErrorV1> {
    if proofs.definition_counts.get(payload.local.index() as usize) != Some(&0) {
        return Err(static_publication_reject_v1());
    }
    let mut snapshot = None;
    for (block, body) in proofs.function.blocks().iter().enumerate() {
        proofs.charge(1)?;
        for (statement, item) in body.statements().iter().enumerate() {
            proofs.charge(1)?;
            let SemanticStatementKindV1::Assign(assignment) = item.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(origin)) =
                assignment.value().kind()
            else {
                continue;
            };
            if origin.local() != payload.local
                || origin.ty() != payload.ty
                || !origin.projections().is_empty()
            {
                continue;
            }
            let destination = assignment.destination();
            let declaration = &proofs.function.locals()[destination.local().index() as usize];
            if destination.ty() != payload.ty
                || !destination.projections().is_empty()
                || declaration.ty() != payload.ty
                || declaration.role() != SemanticLocalRoleV1::Temporary
                || proofs
                    .definition_counts
                    .get(destination.local().index() as usize)
                    != Some(&1)
                || snapshot
                    .replace(StaticPublicationMetadataSnapshotV1 {
                        local: destination.local(),
                        block,
                        statement,
                    })
                    .is_some()
            {
                return Err(static_publication_reject_v1());
            }
        }
    }
    let Some(snapshot) = snapshot else {
        return Ok(None);
    };
    if !indexed_atomic_block_acyclic_v1(proofs, snapshot.block)? {
        return Err(static_publication_reject_v1());
    }
    let definition = ScalarAssignmentSiteV1 {
        block: snapshot.block,
        statement: snapshot.statement,
    };
    let mut length_reads = 0usize;
    // No stored per-use CFG tables: every query uses the existing metered proof
    // context, and the exhaustive custody audit closes all remaining uses.
    for block in 0..proofs.function.blocks().len() {
        proofs.charge(1)?;
        for statement in 0..proofs.function.blocks()[block].statements().len() {
            proofs.charge(1)?;
            let SemanticStatementKindV1::Assign(assignment) =
                proofs.function.blocks()[block].statements()[statement].kind()
            else {
                continue;
            };
            let SemanticRvalueKindV1::Borrow { kind, place } = assignment.value().kind() else {
                continue;
            };
            if place.local() != snapshot.local {
                continue;
            }
            let destination = assignment.destination();
            let receiver = destination.local();
            if *kind != SemanticBorrowKindV1::Shared
                || !place.projections().is_empty()
                || !destination.projections().is_empty()
                || !consumed_read_only_shared_reference_v1(
                    proofs.types,
                    destination.ty(),
                    payload.ty,
                )
                || proofs.definition_counts.get(receiver.index() as usize) != Some(&1)
                || proofs.function.locals()[receiver.index() as usize].role()
                    != SemanticLocalRoleV1::Temporary
                || !proofs.assignment_dominates_use(definition, block, statement)?
                || !indexed_atomic_block_acyclic_v1(proofs, block)?
            {
                return Err(static_publication_reject_v1());
            }
            let borrow = ScalarAssignmentSiteV1 { block, statement };
            for use_block in 0..proofs.function.blocks().len() {
                proofs.charge(1)?;
                let body = &proofs.function.blocks()[use_block];
                let SemanticTerminatorKindV1::Call(call) = body.terminator().kind() else {
                    continue;
                };
                if call.arguments().len() == 1
                    && raw_operand_place(&call.arguments()[0]).is_some_and(|place| {
                        place.local() == receiver && place.projections().is_empty()
                    })
                    && matches!(callables.get(call.callee().index() as usize),
                        Some(SemanticCallableDeclV1::CompilerIntrinsic {
                            operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceLen { disjoint_slice, .. }, ..
                        }) if *disjoint_slice == payload.ty)
                {
                    let statement_count = body.statements().len();
                    if !proofs.assignment_dominates_use(borrow, use_block, statement_count)? {
                        return Err(static_publication_reject_v1());
                    }
                    length_reads += 1;
                }
            }
        }
    }
    if length_reads == 0 {
        return Err(static_publication_reject_v1());
    }
    Ok(Some(snapshot))
}
