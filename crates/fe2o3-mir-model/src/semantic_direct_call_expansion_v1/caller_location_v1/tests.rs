use super::*;

#[path = "literal_divisor_v1_tests.rs"]
mod literal_divisor_v1_tests;

const LOCATION: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const LOCATION_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);

fn location_types(frozen: bool) -> Vec<SemanticTypeDeclV1> {
    let mut types = types();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity(4)),
        SemanticLayoutIdentityV1::from_sha256(identity(4)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(24),
            8,
            SemanticBackendReprV1::memory(true),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    ));
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(identity(5)),
            SemanticLayoutIdentityV1::from_sha256(identity(5)),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    LOCATION,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen },
                        if frozen { 24 } else { 0 },
                        8,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity(6)),
        SemanticLayoutIdentityV1::from_sha256(identity(6)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
    ));
    types
}

fn with_hidden(function: &SemanticFunctionDeclV1, frozen: bool) -> SemanticFunctionDeclV1 {
    let original = function.abi();
    let mut arguments = original.arguments().to_vec();
    arguments.push(SemanticAbiArgumentV1::hidden(
        SemanticAbiHiddenArgumentRoleV1::CallerLocation,
        SemanticAbiValueV1::new(
            LOCATION_REF,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(
                        frozen,
                        frozen.then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                        true,
                        frozen,
                        false,
                        true,
                    ),
                    SemanticAbiExtensionV1::None,
                    if frozen { 24 } else { 0 },
                    Some(8),
                )
                .unwrap(),
            ),
        ),
    ));
    let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        original.identity(),
        original.layout_identity(),
        original.canon_abi(),
        original.extern_abi(),
        original.can_unwind(),
        original.c_variadic(),
        original.fixed_count(),
        original.source_input_types().to_vec(),
        original.source_output_type(),
        arguments,
        original.return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(original.source_argument_ownership().to_vec())
    .unwrap();
    SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        abi,
        function.locals().to_vec(),
        function.entry(),
        function.blocks().to_vec(),
    )
    .unwrap()
}

fn request(
    functions: Vec<SemanticFunctionDeclV1>,
    frozen: bool,
    opaque: Option<SemanticCallableDeclV1>,
) -> InertSemanticMirRequestV1 {
    let has_hidden = functions
        .iter()
        .any(|function| !function.abi().hidden_arguments().is_empty());
    let has_assertion = functions.iter().any(|function| {
        function.blocks().iter().any(|block| {
            matches!(
                block.terminator().kind(),
                SemanticTerminatorKindV1::Assert { .. }
            )
        })
    });
    let mut fixture_types = location_types(frozen);
    if !has_assertion {
        fixture_types.pop();
    }
    if !has_hidden {
        assert!(
            !has_assertion,
            "this fixture's bool ID follows its hidden ABI types"
        );
        fixture_types.truncate(3);
    }
    let mut callables = (0..functions.len())
        .map(|index| SemanticCallableDeclV1::defined(function_id(index as u32)))
        .collect::<Vec<_>>();
    callables.extend(opaque);
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(identity(250))),
        fixture_types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![function_id(0)],
    )
    .unwrap()
}

fn admitted(functions: Vec<SemanticFunctionDeclV1>, frozen: bool) -> AdmittedInertSemanticMirV1 {
    request(functions, frozen, None)
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap()
}

fn caller(index: u32, root: bool, callee: u32) -> SemanticFunctionDeclV1 {
    function(
        index,
        root,
        false,
        vec![
            block(
                0,
                vec![],
                call(callee, 1, false, SemanticUnwindActionV1::Unreachable),
            ),
            block(
                1,
                vec![],
                call(callee, 2, true, SemanticUnwindActionV1::Unreachable),
            ),
            block(2, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
}

fn nested(frozen: bool) -> AdmittedInertSemanticMirV1 {
    admitted(
        vec![
            caller(0, true, 1),
            with_hidden(&caller(1, false, 2), frozen),
            with_hidden(&leaf(2), frozen),
        ],
        frozen,
    )
}

fn expand(source: &AdmittedInertSemanticMirV1) -> Result<SemanticCallExpansionV1> {
    SemanticCallExpansionV1::try_new(source, SemanticCallExpansionLimitsV1::default())
}

#[test]
fn caller_location_both_profiles_preserve_source_abi_and_exact_transfers() {
    for frozen in [false, true] {
        let source = nested(frozen);
        let bytes = source.canonical_encoding().to_vec();
        let abis = source
            .functions()
            .iter()
            .map(|function| function.abi().clone())
            .collect::<Vec<_>>();
        let expansion = expand(&source).unwrap();
        expansion.verify_replay(&source).unwrap();
        let root = expansion.root(function_id(0)).unwrap();
        assert_eq!(root.instances().len(), 7);
        let mut transfers = vec![Vec::new(); root.instances().len()];
        let mut stores = 0;
        for (block, origins) in root.body().blocks().iter().zip(root.block_origins()) {
            for (statement, origin) in block.statements().iter().zip(origins.statements()) {
                if let SemanticExpandedStatementOriginV1::ParameterTransfer { callee, argument } =
                    origin
                {
                    let child = &root.instances()[callee.index() as usize];
                    let function = &source.functions()[child.function().index() as usize];
                    assert!((*argument as usize) < function.abi().source_input_types().len());
                    transfers[callee.index() as usize].push(*argument);
                    let parent = &root.instances()[child.parent().unwrap().index() as usize];
                    let original = &source.functions()[parent.function().index() as usize].blocks()
                        [child.call_block().unwrap().index() as usize];
                    let SemanticTerminatorKindV1::Call(call) = original.terminator().kind() else {
                        panic!()
                    };
                    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                        panic!()
                    };
                    let expected = remap::operand(
                        &call.arguments()[*argument as usize],
                        parent,
                        &mut Budget::new(SemanticCallExpansionLimitsV1::default()).unwrap(),
                    )
                    .unwrap();
                    assert_eq!(
                        assignment.value().kind(),
                        &SemanticRvalueKindV1::Use(expected)
                    );
                    assert_eq!(
                        assignment.destination().ty(),
                        function.abi().source_input_types()[*argument as usize]
                    );
                }
                stores += usize::from(matches!(
                    statement.kind(),
                    SemanticStatementKindV1::Store(_)
                ));
            }
        }
        assert_eq!(stores, 4);
        for arguments in &transfers[1..] {
            assert_eq!(arguments, &[0, 1]);
        }
        assert_eq!(source.canonical_encoding(), bytes);
        assert_eq!(
            source
                .functions()
                .iter()
                .map(|function| function.abi().clone())
                .collect::<Vec<_>>(),
            abis
        );
        assert!(!expansion.grants_proof_or_artifact_authority());
        let evidence = InertCanonicalSemanticCallExpansionEvidenceV1::from_checked_expansion(
            &source, &expansion,
        )
        .unwrap();
        let decoded =
            InertCanonicalSemanticCallExpansionEvidenceV1::decode(evidence.canonical_bytes())
                .unwrap();
        decoded
            .verify_against_checked_expansion(&source, &expansion)
            .unwrap();
        assert!(!decoded.grants_proof_or_artifact_authority());
    }
}

#[test]
fn caller_location_profile_substitution_invalidates_expansion_and_evidence() {
    let first = nested(false);
    let second = nested(true);
    let expansion = expand(&first).unwrap();
    assert!(expansion.verify_replay(&second).is_err());
    let evidence =
        InertCanonicalSemanticCallExpansionEvidenceV1::from_checked_expansion(&first, &expansion)
            .unwrap();
    assert!(
        evidence
            .verify_against_checked_expansion(&second, &expand(&second).unwrap())
            .is_err()
    );
}

#[test]
fn caller_location_crossed_physical_attributes_still_fail_admission() {
    for frozen in [false, true] {
        let request = request(
            vec![caller(0, true, 1), with_hidden(&leaf(1), !frozen)],
            frozen,
            None,
        );
        assert_eq!(
            request
                .admit_current_production(SemanticMirLimitsV1::default())
                .unwrap_err(),
            SemanticMirErrorV1::InvalidFunctionAbi
        );
    }
}

fn observing_assertion(index: u32, unreachable: bool) -> SemanticFunctionDeclV1 {
    let assertion = block(
        u32::from(unreachable),
        vec![],
        SemanticTerminatorKindV1::Assert {
            condition: SemanticOperandV1::Constant(SemanticConstantV1::new(
                BOOL,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 1).unwrap()),
            )),
            expected: true,
            message: SemanticAssertMessageV1::BoundsCheck {
                length: SemanticOperandV1::Copy(place(2, U32)),
                index: SemanticOperandV1::Copy(place(2, U32)),
            },
            target: edge(SemanticEdgeRoleV1::AssertSuccess, u32::from(!unreachable)),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
    );
    let blocks = if unreachable {
        vec![
            block(0, vec![], SemanticTerminatorKindV1::Return),
            assertion,
        ]
    } else {
        vec![
            assertion,
            block(1, vec![], SemanticTerminatorKindV1::Return),
        ]
    };
    function(index, false, false, blocks)
}

#[test]
fn caller_location_assertions_reject_even_when_constant_or_unreachable() {
    for unreachable in [false, true] {
        let source = admitted(
            vec![
                caller(0, true, 1),
                with_hidden(&observing_assertion(1, unreachable), false),
            ],
            false,
        );
        assert!(
            matches!(expand(&source), Err(SemanticCallExpansionErrorV1::Unsupported { function, reason: "caller-location frame contains an observing assertion", .. }) if function == function_id(1))
        );
    }
}

#[test]
fn caller_location_nested_observer_cannot_inherit_unchecked_context() {
    let source = admitted(
        vec![
            caller(0, true, 1),
            with_hidden(&caller(1, false, 2), false),
            with_hidden(&observing_assertion(2, false), false),
        ],
        false,
    );
    assert!(
        matches!(expand(&source), Err(SemanticCallExpansionErrorV1::Unsupported { function, reason: "caller-location frame contains an observing assertion", .. }) if function == function_id(2))
    );
}

fn cold_path() -> SemanticCallableDeclV1 {
    let binding = SemanticNonBodyCallableBindingV1::new(
        SemanticFunctionIdentityV1::from_sha256(identity(3)),
        SemanticItemDefinitionIdentityV1::from_sha256(identity(3)),
        SemanticMonomorphizationIdentityV1::from_sha256(identity(3)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(identity(3)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(identity(3)),
        provenance(),
        SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(identity(3)),
            SemanticLayoutIdentityV1::from_sha256(identity(3)),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![],
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap(),
    );
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding,
        operation: SemanticCompilerIntrinsicOperationV1::ColdPath,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(identity(3)),
    }
}

#[test]
fn caller_location_opaque_calls_reject_without_changing_nonhidden_calls() {
    let helper = function(
        1,
        false,
        false,
        vec![
            block(
                0,
                vec![],
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(2),
                        vec![],
                        Some(SemanticCallDestinationV1::new(
                            place(3, UNIT),
                            edge(SemanticEdgeRoleV1::CallReturn, 1),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
            block(1, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    for hidden in [false, true] {
        let helper = if hidden {
            with_hidden(&helper, false)
        } else {
            helper.clone()
        };
        let source = request(vec![caller(0, true, 1), helper], false, Some(cold_path()))
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap();
        if hidden {
            assert!(matches!(
                expand(&source),
                Err(SemanticCallExpansionErrorV1::Unsupported {
                    reason: "caller-location frame reaches an opaque call",
                    ..
                })
            ));
        } else {
            expand(&source).unwrap().verify_replay(&source).unwrap();
        }
    }
}

#[test]
fn caller_location_audit_remains_bounded_and_recursion_still_rejects() {
    let source = nested(false);
    let work = expand(&source).unwrap().work_units();
    assert!(
        SemanticCallExpansionV1::try_new(
            &source,
            SemanticCallExpansionLimitsV1 {
                work,
                ..SemanticCallExpansionLimitsV1::default()
            }
        )
        .is_ok()
    );
    assert!(matches!(
        SemanticCallExpansionV1::try_new(
            &source,
            SemanticCallExpansionLimitsV1 {
                work: work - 1,
                ..SemanticCallExpansionLimitsV1::default()
            }
        ),
        Err(SemanticCallExpansionErrorV1::Limit(
            SemanticCallExpansionResourceV1::Work
        ))
    ));
    assert!(matches!(
        SemanticCallExpansionV1::try_new(
            &source,
            SemanticCallExpansionLimitsV1 {
                depth: 1,
                ..SemanticCallExpansionLimitsV1::default()
            }
        ),
        Err(SemanticCallExpansionErrorV1::Limit(
            SemanticCallExpansionResourceV1::Depth
        ))
    ));
    let recursive = admitted(
        vec![caller(0, true, 1), with_hidden(&caller(1, false, 1), false)],
        false,
    );
    assert!(matches!(
        expand(&recursive),
        Err(SemanticCallExpansionErrorV1::Unsupported {
            reason: "recursive defined call",
            ..
        })
    ));
}
