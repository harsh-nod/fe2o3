mod reference_call_replay_v26 {
    use super::*;
    include!("production_source_reference_call_replay_v26_tests.rs");
}

#[test]
fn original_global_expression_namespace_keeps_private_reads_and_legacy_values_separate() {
    let completed = std::cell::Cell::new(false);
    with_entry_fixture_v18(
        private_entry_neutral_owner_v20,
        |original, optimized, budget| {
            slice_view_v1::test_global_expression_namespace_v23(
                original, optimized, budget, &completed,
            )
        },
    )
    .unwrap();
    assert!(completed.get());
}

#[test]
fn original_global_store_arithmetic_completes_exact_native_value_join() {
    for mode in [
        DescriptorRoleSourceV18::Constant,
        DescriptorRoleSourceV18::ReadValue,
        DescriptorRoleSourceV18::Arithmetic,
    ] {
        let owner = issued_descriptor_role_owner_v18(mode);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let counts = std::cell::Cell::new([0; 2]);
        let completed = std::cell::Cell::new(false);
        let result = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                slice_view_v1::test_global_source_expressions_v23(
                    original, optimized, budget, &counts, &completed,
                )
            },
        )
        .0;
        assert!(result.is_ok(), "{mode:?}: {result:?}");
        assert!(completed.get(), "{mode:?}: native consumer completion");
        assert_eq!(counts.get(), [1, 1]);
    }
}

fn global_expression_call_v23(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    destination: SemanticPlaceV1,
    next: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                destination,
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(next),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn global_expression_helper_owner_v23(nested: bool) -> ProductionSemanticSsaOwnerV1 {
    global_expression_helper_operation_owner_v23(nested, SemanticBinaryOpV1::Add)
}

fn global_expression_helper_operation_owner_v23(
    nested: bool,
    operation: SemanticBinaryOpV1,
) -> ProductionSemanticSsaOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::*;
    let base = issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::Arithmetic);
    let source = base.source_semantic();
    let original = &source.functions()[0];
    let unit = SemanticTypeIdV1::from_index(0);
    let scalar = SemanticTypeIdV1::from_index(1);
    let reference = original.locals()[7].ty();
    assert!(matches!(source.types()[reference.index() as usize].shape(),
        SemanticTypeShapeV1::Pointer(pointer) if pointer.kind() == SemanticPointerKindV1::Reference
            && pointer.mutability() == SemanticMutabilityV1::Mutable
            && pointer.pointee() == scalar));
    let helpers = 1 + u32::from(nested);
    let mut locals = original.locals().to_vec();
    let destination = locals.len() as u32;
    locals.push(local(85, unit, SemanticLocalRoleV1::Temporary));
    let mut blocks = original.blocks().to_vec();
    for block in &mut blocks {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let terminator = SemanticTerminatorV1::new(
            block.terminator().source(),
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(call.callee().index() + helpers),
                    call.arguments().to_vec(),
                    call.destination().cloned(),
                    call.unwind(),
                )
                .unwrap(),
            ),
        );
        *block = SemanticBasicBlockV1::new(
            block.identity(),
            block.source(),
            block.statements().to_vec(),
            terminator,
        )
        .unwrap();
    }
    assert_eq!(blocks[3].statements().len(), 4);
    // The real issued loan and captured read cross an ordinary helper call.
    // Its arithmetic Store occurs in the helper, not in a detached KIR fixture.
    blocks[3] = SemanticBasicBlockV1::new(
        blocks[3].identity(),
        blocks[3].source(),
        blocks[3].statements()[..2].to_vec(),
        SemanticTerminatorV1::new(
            blocks[3].terminator().source(),
            global_expression_call_v23(
                1,
                vec![
                    SemanticOperandV1::Move(place(7, reference)),
                    SemanticOperandV1::Copy(place(8, scalar)),
                ],
                place(destination, unit),
                4,
            ),
        ),
    )
    .unwrap();
    let root = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let plain = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let unique = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(true, None, true, false, false, true),
        SemanticAbiExtensionV1::None,
        4,
        Some(4),
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([100; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        2,
        vec![
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                reference,
                SemanticAbiPassModeV1::Direct(unique),
            )),
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                scalar,
                SemanticAbiPassModeV1::Direct(plain),
            )),
        ],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
        SemanticSourceArgumentOwnershipV1::ByValue,
    ])
    .unwrap();
    let helper = |tag: u8, forward: bool| {
        let statements = if forward {
            vec![]
        } else {
            let pointer = SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(1),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, scalar)
                        .unwrap(),
                ],
                scalar,
            )
            .unwrap();
            vec![
                assign(
                    place(3, scalar),
                    SemanticRvalueKindV1::Binary {
                        operation,
                        left: SemanticOperandV1::Copy(place(2, scalar)),
                        right: SemanticOperandV1::Constant(SemanticConstantV1::new(
                            scalar,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(1, 4).unwrap(),
                            ),
                        )),
                    },
                ),
                SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                        pointer,
                        SemanticOperandV1::Copy(place(3, scalar)),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    )),
                ),
            ]
        };
        let blocks = if forward {
            vec![
                block(
                    tag + 5,
                    statements,
                    global_expression_call_v23(
                        2,
                        vec![
                            SemanticOperandV1::Move(place(1, reference)),
                            SemanticOperandV1::Copy(place(2, scalar)),
                        ],
                        place(4, unit),
                        1,
                    ),
                ),
                block(tag + 6, vec![], SemanticTerminatorKindV1::Return),
            ]
        } else {
            vec![block(tag + 5, statements, SemanticTerminatorKindV1::Return)]
        };
        function(
            tag,
            SemanticFunctionRoleV1::InternalHelper,
            abi.clone(),
            vec![
                local(tag + 1, unit, SemanticLocalRoleV1::Return),
                local(tag + 2, reference, SemanticLocalRoleV1::Argument(0)),
                local(tag + 3, scalar, SemanticLocalRoleV1::Argument(1)),
                local(tag + 4, scalar, SemanticLocalRoleV1::Temporary),
                local(tag + 7, unit, SemanticLocalRoleV1::Temporary),
            ],
            blocks,
        )
    };
    let mut functions = vec![root, helper(100, nested)];
    if nested {
        functions.push(helper(120, false));
    }
    let mut callables: Vec<_> = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect();
    callables.extend_from_slice(&source.callables()[1..]);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        functions,
        callables,
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn original_global_store_value_follows_actual_mutable_loan_and_nested_scalar_callers() {
    for nested in [false, true] {
        let owner = global_expression_helper_owner_v23(nested);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let counts = std::cell::Cell::new([0; 2]);
        let completed = std::cell::Cell::new(false);
        let result = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                slice_view_v1::test_global_source_expressions_v23(
                    original, optimized, budget, &counts, &completed,
                )
            },
        )
        .0;
        assert!(result.is_ok(), "nested={nested}: {result:?}");
        assert!(completed.get());
        assert_eq!(counts.get(), [1, 1]);
    }
}

#[test]
fn original_global_store_completion_rejects_substituted_instance_value_root_and_guard() {
    for fault in 0..5 {
        let owner = global_expression_helper_owner_v23(true);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let completed = std::cell::Cell::new(false);
        let result = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                slice_view_v1::test_global_source_expression_pair_v23(
                    original, optimized, budget, fault, &completed,
                )
            },
        )
        .0;
        assert!(completed.get(), "fault={fault}: {result:?}");
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "pending global native substituted checked source pair",
            ))
        ));
    }
}

#[test]
fn original_global_store_expression_scope_settles_exact_and_one_short_work() {
    let run = |remaining| {
        let owner = global_expression_helper_owner_v23(true);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let used = std::cell::Cell::new(0);
        let settled = std::cell::Cell::new(false);
        let selected = std::cell::Cell::new(None);
        let result = run_descriptor_role_owner_result_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                slice_view_v1::test_global_source_expression_work_v23(
                    original,
                    optimized,
                    budget,
                    OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                    remaining,
                    &used,
                    &settled,
                    &selected,
                )
            },
        )
        .0
        .map_err(|error| match error {
            ProductionSourceOptimizationErrorV18::Source(error) => error,
            ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                    error @ ProductionSourceOwnedViewErrorV18::Binding(
                        "test stops after exact global source expression scope",
                    ),
                ),
            ) => error,
            other => panic!("unexpected exact expression-scope refusal: {other:?}"),
        });
        assert!(settled.get(), "scope settlement must complete: {result:?}");
        (result, used.get(), selected.get())
    };
    let (measured, needed, selected) = run(None);
    assert!(matches!(
        measured,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "test stops after exact global source expression scope",
        ))
    ));
    assert!(needed > 1);
    assert!(selected.is_none());
    let (exact, used, selected) = run(Some(needed));
    assert!(matches!(
        exact,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "test stops after exact global source expression scope",
        ))
    ));
    assert_eq!(used, needed);
    assert!(selected.is_none());
    let (short, _, selected) = run(Some(needed - 1));
    let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error))) = short
    else {
        panic!("one-short original shared Work denial");
    };
    assert_eq!(Some((error.actual(), error.limit())), selected);
    assert_eq!(error.limit(), OPTIMIZED_SOURCE_WORK_LIMIT_V18);
}

#[test]
fn original_global_store_expression_authenticates_foreign_failed_account_before_headers() {
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.set(self.0);
        }
    }
    let _restore = Restore(DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.replace(None));
    let owner = issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::Arithmetic);
    let abi = issued_descriptor_role_abi_v18(&owner);
    let completed = std::cell::Cell::new(false);
    let result = run_descriptor_role_owner_with_abi_v18(
        owner,
        abi,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            let result = slice_view_v1::test_global_source_expression_entry_v23(
                original,
                optimized,
                budget,
                MODULE_LIMIT,
                0,
                &completed,
            );
            DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.set(Some(budget.storage()));
            result
        },
    )
    .0;
    assert!(
        completed.get(),
        "entry/retry checks must finish: {result:?}"
    );
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting,
        ))
    ));
    assert!(DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.get().unwrap() > MODULE_FLOOR);
}

#[test]
fn original_global_store_expression_retains_early_header_and_prior_work_denials() {
    for fault in [1, 2] {
        let owner = issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::Arithmetic);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let completed = std::cell::Cell::new(false);
        let result = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                slice_view_v1::test_global_source_expression_entry_v23(
                    original,
                    optimized,
                    budget,
                    if fault == 1 {
                        MODULE_LIMIT
                    } else {
                        OPTIMIZED_SOURCE_WORK_LIMIT_V18
                    },
                    fault,
                    &completed,
                )
            },
        )
        .0;
        assert!(completed.get(), "fault={fault}: {result:?}");
        match (fault, result) {
            (
                1,
                Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(
                    error,
                ))),
            ) => {
                assert_eq!(
                    (error.actual(), error.limit()),
                    (MODULE_LIMIT + 1, MODULE_LIMIT)
                );
            }
            (
                2,
                Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error))),
            ) => {
                assert_eq!(error.limit(), OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                assert!(error.actual() > OPTIMIZED_SOURCE_WORK_LIMIT_V18);
            }
            (_, error) => panic!("exact entry refusal: {error:?}"),
        }
    }
}

#[test]
fn original_global_store_expression_requires_original_and_optimized_rhs_equality() {
    for output in [false, true] {
        for fault in 0..3 {
            let owner = global_expression_helper_owner_v23(true);
            let abi = issued_descriptor_role_abi_v18(&owner);
            let completed = std::cell::Cell::new(false);
            let result = run_descriptor_role_owner_with_abi_v18(
                owner,
                abi,
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                MODULE_LIMIT,
                |original, optimized, budget| {
                    slice_view_v1::test_global_source_expression_counterfeit_v23(
                        original, optimized, budget, fault, output, &completed,
                    )
                },
            )
            .0;
            assert!(completed.get(), "output={output} fault={fault}: {result:?}");
            let expected = if output {
                "actual optimized scalar expression differs from its original source value"
            } else {
                "actual scalar expression differs from its original source value"
            };
            assert!(
                matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(message)) if message == expected)
            );
        }
    }
}

#[test]
fn original_global_store_expression_keeps_unsupported_source_operators_pending() {
    for operation in [
        SemanticBinaryOpV1::ShiftLeft,
        SemanticBinaryOpV1::ShiftRight,
    ] {
        let owner = global_expression_helper_operation_owner_v23(true, operation);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let counts = std::cell::Cell::new([0; 2]);
        let completed = std::cell::Cell::new(false);
        let attempted = std::cell::Cell::new(false);
        let settled = std::cell::Cell::new(false);
        let result = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                let floor = budget.storage();
                attempted.set(true);
                let result = slice_view_v1::test_global_source_expressions_v23(
                    original, optimized, budget, &counts, &completed,
                );
                assert!(
                    matches!(
                        result,
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "private source expression unsupported arithmetic contract",
                        ))
                    ),
                    "{result:?}"
                );
                assert_eq!(budget.storage(), floor);
                settled.set(true);
                result
            },
        )
        .0;
        assert!(
            attempted.get() && settled.get(),
            "{operation:?}: {result:?}"
        );
        assert!(!completed.get());
        assert_eq!(counts.get(), [0, 0]);
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "private source expression unsupported arithmetic contract",
            ))
        ));
    }
}

#[test]
fn original_global_store_expression_cannot_swallow_callback_resource_denial() {
    for storage in [false, true] {
        let owner = issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::Arithmetic);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let completed = std::cell::Cell::new(false);
        let settled = std::cell::Cell::new(false);
        let selected = std::cell::Cell::new(None);
        let limit = if storage {
            MODULE_LIMIT
        } else {
            OPTIMIZED_SOURCE_WORK_LIMIT_V18
        };
        let result = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                let floor = budget.storage();
                let result = slice_view_v1::test_global_source_expression_callback_denial_v23(
                    original, optimized, budget, storage, limit, &completed, &selected,
                );
                assert_eq!(budget.storage(), floor);
                settled.set(true);
                result
            },
        )
        .0;
        assert!(completed.get() && settled.get(), "{result:?}");
        let observed =
            match result {
                Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(
                    error,
                ))) if storage => (error.actual(), error.limit()),
                Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(
                    error,
                ))) if !storage => (error.actual(), error.limit()),
                other => panic!("outer scope must retain original callback denial: {other:?}"),
            };
        assert_eq!(selected.get(), Some(observed));
        assert_eq!(observed.1, limit);
        assert!(observed.0 > limit);
    }
}
