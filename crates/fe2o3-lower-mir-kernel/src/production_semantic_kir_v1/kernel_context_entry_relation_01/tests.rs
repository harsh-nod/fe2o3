use super::*;
include!(concat!(env!("CARGO_MANIFEST_DIR"),
    "/../fe2o3-pliron/src/production/semantic_ssa/defined_math_results/fixture.rs"));

// Inert component input. Only the production frontend authenticates the HIR
// initializer/use evidence represented by this test's source-binding label.
fn component() -> (ProductionSemanticSsaOwnerV1, ProductionKernelContextEntryTransferV1) {
    let base = source(false, false);
    let root = function(135, abi(133, &[], 0, true), vec![
        local(180, 0, SemanticLocalRoleV1::Return),
        local(181, 1, SemanticLocalRoleV1::Temporary),
        local(182, 3, SemanticLocalRoleV1::Temporary),
        local(183, 4, SemanticLocalRoleV1::Temporary),
    ], vec![
        block(180, vec![], call(2, vec![], 1, 1, 1)),
        block(181, vec![], call(1, vec![SemanticOperandV1::Constant(
            SemanticConstantV1::new(ty(1), SemanticConstantValueV1::ZeroSized),
        )], 0, 0, 2)),
        block(182, vec![], SemanticTerminatorKindV1::Return),
    ], true).with_kernel_entry(base.functions()[0].kernel_entry().unwrap().clone());
    let old = abi(160, &[], 0, false);
    let helper_abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        old.identity(), old.layout_identity(), old.canon_abi(), old.extern_abi(),
        false, false, 1, vec![ty(1)], ty(0), vec![SemanticAbiArgumentV1::source(
            SemanticAbiValueV1::new(ty(1), SemanticAbiPassModeV1::Ignore),
        )], old.return_value().clone(),
    ).unwrap().with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue]).unwrap();
    let helper = function(160, helper_abi, vec![
        local(180, 0, SemanticLocalRoleV1::Return),
        local(181, 1, SemanticLocalRoleV1::Argument(0)),
        local(182, 2, SemanticLocalRoleV1::Temporary),
    ], vec![block(180, vec![SemanticStatementV1::new(location(), SemanticStatementKindV1::Assign(
        SemanticAssignmentV1::new(place(2, 2), SemanticRvalueV1::new(ty(2), SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared, place: place(1, 1),
        })),
    ))], SemanticTerminatorKindV1::Return)], false);
    let callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        terminal(180, abi(180, &[], 1, false), SemanticCompilerIntrinsicOperationV1::KernelContextIssue { context: ty(1) }),
    ];
    let source = InertSemanticMirRequestV1::new_with_callables(base.target(), base.types().to_vec(),
        vec![], vec![], vec![], vec![root, helper], callables, vec![SemanticFunctionIdV1::from_index(0)],
    ).unwrap().admit_current_production(SemanticMirLimitsV1::default()).unwrap();
    let input = ProductionKernelContextEntryTransferV1::new(
        *source.semantic_sha256().as_bytes(), source.functions()[0].identity(), source.functions()[1].identity(),
        SemanticFunctionIdentityV1::from_sha256([180; 32]), source.types()[1].identity(),
        SemanticBlockIdV1::from_index(0), SemanticLocalIdV1::from_index(1), SemanticBlockIdV1::from_index(1), 0, [33; 32],
    );
    let mir = ProductionSemanticMirOwnerV1::try_new(source, Default::default()).unwrap();
    (ProductionSemanticSsaOwnerV1::try_new(mir, Default::default()).unwrap(), input)
}

#[test]
fn shared_entry_relation_retains_original_constant_and_exact_ssa_definitions() {
    let (owner, input) = component();
    let root = SemanticFunctionIdV1::from_index(0);
    let relation = input.checked_ssa_relation(&owner, root, 65_536).unwrap();
    let view = owner.execution_view_for_root(root).unwrap();
    assert!(relation.matches_body(view.body()));
    assert_ne!(relation.issuer_value(), relation.parameter_value());
    assert_eq!(relation.issuer_local().index(), 1);
    assert_eq!(relation.context(), ty(1));
    let SemanticStatementKindV1::Assign(assignment) = view.body().blocks()[relation.block().index() as usize]
        .statements()[relation.statement() as usize].kind() else { panic!() };
    assert_eq!(assignment.destination().local(), relation.destination());
    assert!(matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(value))
        if value.value() == &SemanticConstantValueV1::ZeroSized));
    let (other, _) = component();
    assert!(!relation.matches_body(other.execution_view_for_root(root).unwrap().body()));
    owner.verify_replay().unwrap();
}

#[test]
fn shared_entry_relation_rejects_substituted_source_coordinates_and_empty_evidence() {
    let (owner, _) = component();
    let source = owner.source_semantic();
    for mutation in 0..5 {
        let input = ProductionKernelContextEntryTransferV1::new(
            if mutation == 1 { [99; 32] } else { *source.semantic_sha256().as_bytes() },
            source.functions()[0].identity(),
            if mutation == 2 { SemanticFunctionIdentityV1::from_sha256([99; 32]) } else { source.functions()[1].identity() },
            SemanticFunctionIdentityV1::from_sha256([180; 32]), source.types()[1].identity(),
            SemanticBlockIdV1::from_index(0), SemanticLocalIdV1::from_index(if mutation == 3 { 0 } else { 1 }),
            SemanticBlockIdV1::from_index(1), u32::from(mutation == 4), if mutation == 0 { [0; 32] } else { [33; 32] },
        );
        assert!(matches!(input.checked_ssa_relation(&owner, SemanticFunctionIdV1::from_index(0), 65_536),
            Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. }) if detail == "Context entry source transfer changed"),
            "accepted substituted entry field {mutation}");
    }
}

#[test]
fn shared_entry_relation_preserves_the_work_ceiling() {
    let (owner, input) = component();
    assert!(matches!(input.checked_ssa_relation(&owner, SemanticFunctionIdV1::from_index(0), 0),
        Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
            if detail == "Context entry SSA relation changed or exceeded its work bound"));
}
