//! Hostile inert source mutants at the exact permitted unique-field Copy scope.
//! No sink, issuer, source seal or independently supplied borrow fact is added.
use super::*;

pub(super) fn mutate(
    source: &AdmittedInertSemanticMirV1,
    capture: &Capture,
    functions: &mut [SemanticFunctionDeclV1],
    mutation: Mutation,
) -> Option<SemanticFunctionIdV1> {
    if !matches!(
        mutation,
        Mutation::DuplicateScopedFieldCopy
            | Mutation::DuplicateScopedFieldDefinition
            | Mutation::UncheckedScopedLeafCopy
    ) {
        return None;
    }
    let body = &functions[capture.closure.index() as usize];
    assert!(body.defined_capability_contract().is_none());
    let environments = body
        .locals()
        .iter()
        .enumerate()
        .filter(|(_, local)| local.role() == SemanticLocalRoleV1::Argument(0))
        .map(|(index, _)| SemanticLocalIdV1::from_index(index as u32))
        .collect::<Vec<_>>();
    let [environment] = environments.as_slice() else {
        panic!("one original closure environment argument");
    };
    let mut found = None;
    for (block, b) in body.blocks().iter().enumerate() {
        for (statement, item) in b.statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(a) = item.kind() else {
                continue;
            };
            if a.destination().ty() != capture.reference_type
                || !a.destination().projections().is_empty()
            {
                continue;
            }
            let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = a.value().kind() else {
                continue;
            };
            if place.local() != *environment
                || place.ty() != capture.reference_type
                || !matches!(place.projections(), [field]
                    if field.kind() == SemanticProjectionKindV1::Field(capture.field as u32)
                        && field.result_type() == capture.reference_type)
            {
                continue;
            }
            assert!(
                found.replace((block, statement)).is_none(),
                "one original exact unique field extraction"
            );
        }
    }
    let point = found.expect("the actual closure must expose its exact unique-field Copy");
    let relays = source
        .functions()
        .iter()
        .filter_map(|f| match f.defined_capability_contract() {
            Some(SemanticDefinedCapabilityContractV1::ReusablePhase(record)) => {
                match record.recipe() {
                    R::WithPhase { relay, .. } if relay.closure_function == capture.closure => {
                        Some(relay)
                    }
                    _ => None,
                }
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [relay] = relays.as_slice() else {
        panic!("one actual wrapper for the selected closure");
    };
    assert_ne!(
        point.0 as u32,
        relay.closure_pack_block.index(),
        "field extraction precedes Bind/Finish; do not shift the completion pack coordinate"
    );
    let a = assignment(body, point);
    assert_eq!(a.value().result_type(), capture.reference_type);
    let mut locals = body.locals().to_vec();
    let destination = if matches!(mutation, Mutation::DuplicateScopedFieldDefinition) {
        a.destination().clone()
    } else {
        let local = SemanticLocalIdV1::from_index(u32::try_from(locals.len()).unwrap());
        let identity = locals
            .iter()
            .map(|l| *l.identity().as_bytes())
            .max()
            .unwrap();
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(next_identity(identity)),
            capture.reference_type,
            SemanticLocalRoleV1::Temporary,
            body.source(),
        ));
        SemanticPlaceV1::new(local, vec![], capture.reference_type).unwrap()
    };
    let value = if matches!(mutation, Mutation::UncheckedScopedLeafCopy) {
        // Correct scope and type, but not the exact environment-field route.
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(a.destination().clone()))
    } else {
        // Leave the original extraction and its later Bind consumer untouched.
        // The second field Copy starts while that original unique child is live.
        a.value().kind().clone()
    };
    let mut blocks = body.blocks().to_vec();
    let block = &blocks[point.0];
    let mut statements = block.statements().to_vec();
    statements.insert(point.1 + 1, assigned(block.source(), destination, value));
    blocks[point.0] = SemanticBasicBlockV1::new(
        block.identity(),
        block.source(),
        statements,
        block.terminator().clone(),
    )
    .unwrap();
    functions[capture.closure.index() as usize] = rebuild_function(body, locals, blocks);
    Some(capture.closure)
}

pub(super) fn recommit_closure(
    mut recipe: R,
    changed: Option<SemanticFunctionIdV1>,
    functions: &[SemanticFunctionDeclV1],
    work: &mut u64,
) -> R {
    let R::WithPhase { relay, .. } = &mut recipe else {
        return recipe;
    };
    if changed != Some(relay.closure_function) {
        return recipe;
    }
    let body = &functions[relay.closure_function.index() as usize];
    let (hash, bytes) = canonical_semantic_source_body_sha256_v25(body, *work / 2).unwrap();
    let charged = u64::try_from(bytes).unwrap().checked_mul(2).unwrap();
    *work = work.checked_sub(charged).unwrap();
    assert_ne!(
        hash, relay.closure_body,
        "negative must change the actual closure body"
    );
    // Rebuild the inert hash only. The unchanged constructor validates all
    // completion coordinates, effects and dependencies against this new body.
    relay.closure_body = hash;
    recipe
}
