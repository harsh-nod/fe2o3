// Synthetic component anchors exercise the join only. They are not frontend
// custody; production derives this tuple after replaying the authenticated entry.
fn entry_invocation_anchor_fixture() -> (
    SemanticTypeIdV1,
    SemanticLocalIdV1,
    SemanticKernelCapabilityProvenanceV1,
) {
    (
        CAP_INDEX_CONTEXT,
        SemanticLocalIdV1::from_index(1),
        capability_index_provenance(None),
    )
}

fn entry_invocation_join_fixture(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    anchor: Option<(
        SemanticTypeIdV1,
        SemanticLocalIdV1,
        SemanticKernelCapabilityProvenanceV1,
    )>,
) -> Result<Option<SemanticKernelCapabilityProvenanceV1>, ProductionRankedProjectionErrorV1> {
    let root = capability_index_provenance(None);
    root_invocation_provenance_v1(
        types,
        callables,
        function,
        root.root(),
        root.kernel_binding(),
        anchor,
    )
}

#[test]
fn entry_invocation_provenance_needs_no_unrelated_global_binder() {
    let (types, mut callables, function) = capability_index_fixture();
    callables[1] = compiler_intrinsic_callable(SemanticCompilerIntrinsicOperationV1::FabsF32);
    let expected = capability_index_provenance(None);
    let joined = entry_invocation_join_fixture(
        &types,
        &callables,
        &function,
        Some(entry_invocation_anchor_fixture()),
    )
    .unwrap();
    assert_eq!(joined, Some(expected));
    let SemanticTerminatorKindV1::Call(call) = function.blocks()[2].terminator().kind() else {
        panic!("the component has a real invocation-index call");
    };
    validate_capability_invocation_index_v1(&types, &callables[2], call, joined).unwrap();
    assert_incomplete(
        entry_invocation_join_fixture(&types, &callables, &function, None),
        "an invocation index lacks an independent root context bind",
    );
}

#[test]
fn entry_invocation_provenance_agrees_with_existing_global_anchor() {
    let (types, callables, function) = capability_index_fixture();
    assert_eq!(
        entry_invocation_join_fixture(
            &types,
            &callables,
            &function,
            Some(entry_invocation_anchor_fixture()),
        )
        .unwrap(),
        entry_invocation_join_fixture(&types, &callables, &function, None).unwrap(),
    );
}

#[test]
fn entry_invocation_provenance_rejects_wrong_context_issuer_root_and_binding() {
    let (types, mut callables, function) = capability_index_fixture();
    callables[1] = compiler_intrinsic_callable(SemanticCompilerIntrinsicOperationV1::FabsF32);
    for mutation in 0..4 {
        let mut anchor = entry_invocation_anchor_fixture();
        match mutation {
            0 => anchor.0 = CAP_INDEX_INVOCATION,
            1 => anchor.1 = SemanticLocalIdV1::from_index(2),
            2 => anchor.2 = capability_index_provenance(Some(0)),
            _ => anchor.2 = capability_index_provenance(Some(1)),
        }
        assert_incomplete(
            entry_invocation_join_fixture(&types, &callables, &function, Some(anchor)),
            "an invocation index changed its authenticated Context entry anchor",
        );
    }
    callables[0] = compiler_intrinsic_callable(SemanticCompilerIntrinsicOperationV1::FabsF32);
    assert_incomplete(
        entry_invocation_join_fixture(
            &types,
            &callables,
            &function,
            Some(entry_invocation_anchor_fixture()),
        ),
        "an invocation index changed its authenticated Context entry anchor",
    );
}

#[test]
fn entry_invocation_provenance_still_requires_one_exact_issuance() {
    let (types, mut callables, function) = capability_index_fixture();
    callables[1] =
        compiler_intrinsic_callable(SemanticCompilerIntrinsicOperationV1::KernelContextIssue {
            context: CAP_INDEX_CONTEXT,
        });
    assert_incomplete(
        entry_invocation_join_fixture(
            &types,
            &callables,
            &function,
            Some(entry_invocation_anchor_fixture()),
        ),
        "an invocation index requires one exact root context issuance",
    );
}

#[test]
fn entry_invocation_provenance_does_not_hide_conflicting_global_binding() {
    let (types, callables, function) = capability_index_fixture();
    for mutation in 0..8 {
        let mut changed = callables.clone();
        let SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindReadOnly {
                    provenance,
                    source_identity,
                    ..
                },
            ..
        } = &mut changed[1]
        else {
            unreachable!()
        };
        if mutation < 7 {
            *provenance = capability_index_provenance(Some(mutation));
        } else {
            *source_identity = SemanticFunctionIdentityV1::from_sha256(bytes(201));
        }
        assert_incomplete(
            entry_invocation_join_fixture(
                &types,
                &changed,
                &function,
                Some(entry_invocation_anchor_fixture()),
            ),
            "an invocation index has conflicting root context provenance",
        );
    }
}

#[test]
fn entry_invocation_provenance_is_not_taken_from_its_consumer() {
    let (types, mut callables, function) = capability_index_fixture();
    callables[1] = compiler_intrinsic_callable(SemanticCompilerIntrinsicOperationV1::FabsF32);
    let SemanticTerminatorKindV1::Call(call) = function.blocks()[2].terminator().kind() else {
        unreachable!()
    };
    for mutation in 0..8 {
        let mut changed = callables.clone();
        let SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::CapabilityInvocationIndex1d {
                    provenance,
                    source_identity,
                    ..
                },
            ..
        } = &mut changed[2]
        else {
            unreachable!()
        };
        if mutation < 7 {
            *provenance = capability_index_provenance(Some(mutation));
        } else {
            *source_identity = SemanticFunctionIdentityV1::from_sha256(bytes(201));
        }
        let joined = entry_invocation_join_fixture(
            &types,
            &changed,
            &function,
            Some(entry_invocation_anchor_fixture()),
        )
        .unwrap();
        assert_eq!(joined, Some(capability_index_provenance(None)));
        assert_incomplete(
            validate_capability_invocation_index_v1(&types, &changed[2], call, joined),
            "an invocation index changed its root provenance or source binding",
        );
    }
}

#[test]
fn entry_invocation_provenance_is_unneeded_without_invocation_or_memory_calls() {
    let (types, callables, function) = capability_index_fixture();
    let callables = callables
        .iter()
        .map(|_| compiler_intrinsic_callable(SemanticCompilerIntrinsicOperationV1::FabsF32))
        .collect::<Vec<_>>();
    for anchor in [None, Some(entry_invocation_anchor_fixture())] {
        assert_eq!(
            entry_invocation_join_fixture(&types, &callables, &function, anchor).unwrap(),
            None,
        );
    }
}
