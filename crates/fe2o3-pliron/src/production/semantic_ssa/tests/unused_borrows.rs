use super::*;

#[test]
fn numerical_policy_receiver_requires_the_exact_reference_type_and_position() {
    use fe2o3_mir_model::semantic_mir_v1::*;
    let reference = SemanticTypeIdV1::from_index(1);
    let capability = SemanticTypeIdV1::from_index(2);
    let provenance = SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1::from_index(0),
        SemanticKernelBindingIdentityV1::from_sha256([1; 32]),
        SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([2; 32]),
        SemanticTypeIdentityV1::from_sha256([3; 32]),
        SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([4; 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([5; 32]),
        SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([6; 32]),
    )
    .unwrap();
    let operation = SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
        contract: SemanticExecutionCapabilityContractV1::new_kernel_scoped(
            SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue {
                context: reference,
                capability,
                policy: SemanticTypeIdentityV1::from_sha256([7; 32]),
            },
            SemanticExecutionCapabilitySignatureV1::new(&[reference], capability).unwrap(),
            provenance,
            SemanticFunctionIdentityV1::from_sha256([8; 32]),
        )
        .unwrap(),
    };
    for (position, observed, expected) in [
        (0, reference, true),
        (1, reference, false),
        (0, capability, false),
    ] {
        let mut arguments = Vec::new();
        if position != 0 {
            arguments.push(SemanticOperandV1::Copy(test_typed_place(1, 0)));
        }
        arguments.push(SemanticOperandV1::Copy(test_typed_place(
            2,
            observed.index(),
        )));
        let function = test_function(vec![test_block(
            151,
            vec![test_borrow(2, 1)],
            test_call(0, arguments, None),
        )]);
        let callables = [test_operation_callable(
            function.abi().clone(),
            operation.clone(),
            80,
        )];
        assert_eq!(
            transparent_borrow_sites_v1(&function, &callables).len(),
            usize::from(expected)
        );
    }
}

fn alias(destination: u32, source: u32, moved: bool) -> SemanticStatementV1 {
    let place = test_scalar_place(source);
    test_assign(
        destination,
        if moved {
            SemanticOperandV1::Move(place)
        } else {
            SemanticOperandV1::Copy(place)
        },
    )
}

#[test]
fn unused_borrow_and_closed_argument_transport_are_transparent() {
    for moved in [false, true] {
        let function = test_function(vec![
            test_block(
                140,
                vec![test_borrow(2, 1)],
                SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            test_block(
                141,
                vec![alias(3, 2, moved)],
                SemanticTerminatorKindV1::Return,
            ),
        ]);
        assert_eq!(transparent_borrow_sites_v1(&function, &[]).len(), 1);
        assert!(source_is_promotable(&function, &[]));
    }
    let function = test_function(vec![test_block(
        142,
        vec![test_borrow(2, 1)],
        SemanticTerminatorKindV1::Return,
    )]);
    assert_eq!(transparent_borrow_sites_v1(&function, &[]).len(), 1);
}

#[test]
fn unused_reference_reborrow_chain_is_transparent() {
    let function = test_function(vec![test_block(
        143,
        vec![test_borrow(2, 1), test_reborrow(3, 2)],
        SemanticTerminatorKindV1::Return,
    )]);
    assert_eq!(transparent_borrow_sites_v1(&function, &[]).len(), 2);
}

#[test]
fn intrinsic_reborrow_cannot_hide_an_intermediate_reference_alias() {
    for moved in [false, true] {
        let function = test_function_with_reference_locals(
            vec![test_block(
                150,
                vec![test_borrow(2, 1), alias(3, 2, moved), test_reborrow(4, 3)],
                test_call(0, vec![SemanticOperandV1::Copy(test_scalar_place(4))], None),
            )],
            5,
        );
        let callables = [test_intrinsic_callable(function.abi().clone())];
        assert!(transparent_borrow_sites_v1(&function, &callables).is_empty());
        assert!(!source_is_promotable(&function, &callables));
    }
}

#[test]
fn reference_return_is_observable_with_or_without_a_transport() {
    for statements in [
        vec![test_borrow(0, 1)],
        vec![test_borrow(2, 1), alias(0, 2, true)],
    ] {
        let function = test_function(vec![test_block(
            144,
            statements,
            SemanticTerminatorKindV1::Return,
        )]);
        assert!(transparent_borrow_sites_v1(&function, &[]).is_empty());
    }
}

#[test]
fn unused_transport_rejects_escape_and_reference_observation() {
    let function = test_function(vec![test_block(
        145,
        vec![test_borrow(2, 1), alias(3, 2, true)],
        test_call(0, vec![SemanticOperandV1::Copy(test_scalar_place(3))], None),
    )]);
    assert!(transparent_borrow_sites_v1(&function, &[]).is_empty());
    let callables = [test_intrinsic_callable(function.abi().clone())];
    assert!(transparent_borrow_sites_v1(&function, &callables).is_empty());
    let function = test_function(vec![test_block(
        146,
        vec![test_borrow(2, 1), test_typed_borrow(3, 1, 2, 1)],
        SemanticTerminatorKindV1::Return,
    )]);
    assert!(transparent_borrow_sites_v1(&function, &[]).is_empty());
}

#[test]
fn unused_transport_rejects_forks_redefinitions_and_cycles() {
    for statements in [
        vec![test_borrow(2, 1), alias(3, 2, false), alias(4, 2, false)],
        vec![test_borrow(2, 1), alias(3, 2, true), test_borrow(3, 1)],
        vec![test_borrow(2, 1), alias(3, 2, true), alias(2, 3, true)],
    ] {
        let function = test_function_with_reference_locals(
            vec![test_block(
                147,
                statements,
                SemanticTerminatorKindV1::Return,
            )],
            5,
        );
        assert!(transparent_borrow_sites_v1(&function, &[]).is_empty());
    }
}

#[test]
fn unused_transport_does_not_supply_an_undefined_or_killed_source() {
    let undefined = test_implicit_scope_function(
        0,
        vec![test_block(
            148,
            vec![test_typed_borrow(2, 2, 1, 0)],
            SemanticTerminatorKindV1::Return,
        )],
    );
    assert!(transparent_borrow_sites_v1(&undefined, &[]).is_empty());
    let (input, implicit, _) = implicit_scope_ssa_input(&undefined, &implicit_scope_types(), &[]);
    assert!(!input.promotable()[1]);
    assert!(implicit.is_empty());

    let killed = test_function(vec![test_block(
        149,
        vec![
            SemanticStatementV1::new(
                fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)),
            ),
            test_borrow(2, 1),
        ],
        SemanticTerminatorKindV1::Return,
    )]);
    assert!(
        plan_semantic_function_ssa_with_module_v1(
            SemanticFunctionIdV1::from_index(0),
            &killed,
            &test_types(false),
            &[],
            ProductionSemanticSsaLimitsV1::default()
        )
        .is_err()
    );
}
