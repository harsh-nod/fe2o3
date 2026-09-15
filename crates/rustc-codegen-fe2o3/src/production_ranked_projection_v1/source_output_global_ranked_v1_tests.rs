// Included inside the genuine Global source fixture module.
#[derive(Clone, Copy, Debug)]
enum GlobalWriteExpressionShapeV1 {
    Parameter,
    ParameterArithmetic,
    LoadArithmetic,
}

const G_SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);

fn global_ranked_slice_types_v1() -> Vec<SemanticTypeDeclV1> {
    let reference = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(237)),
        SemanticLayoutIdentityV1::from_sha256(bytes(237)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::scalar_pair(
                SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                ),
                SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 64, 8),
                    SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                ),
            ),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                G_SLICE,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::SliceLength,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                    0,
                    4,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    let slice = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(238)),
        SemanticLayoutIdentityV1::from_sha256(bytes(238)),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            4,
            SemanticFieldsShapeV1::array(4, 0),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(false),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Slice { element: A_U32 },
    );
    let mut types = assertion_types();
    types.extend([reference, slice]);
    types
}

fn global_ranked_slice_place_v1() -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, G_SLICE).unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(8)),
                A_U32,
            )
            .unwrap(),
        ],
        A_U32,
    )
    .unwrap()
}

fn global_ranked_slice_store_v1(value: u32) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        global_ranked_slice_place_v1(),
        SemanticRvalueV1::new(
            A_U32,
            SemanticRvalueKindV1::Use(typed_operand(value, A_U32)),
        ),
    )))
}

fn genuine_dynamic_global_source_v1(
    shape: GlobalWriteExpressionShapeV1,
    lanes: u32,
) -> ProductionPreRankedKirOwnerV1 {
    genuine_global_source_variant_v1(shape, lanes, false)
}

fn genuine_global_source_variant_v1(
    shape: GlobalWriteExpressionShapeV1,
    lanes: u32,
    dead: bool,
) -> ProductionPreRankedKirOwnerV1 {
    let first = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(true, None, true, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        Some(4),
    )
    .unwrap();
    let second = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let pair = || {
        SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            G_POINTER,
            SemanticAbiPassModeV1::Pair { first, second },
        ))
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(236)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        3,
        vec![
            pair(),
            pair(),
            SemanticAbiArgumentV1::source(neutral_plain_direct_abi_value_v1(A_U32)),
        ],
        SemanticAbiValueV1::new(A_UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
        SemanticSourceArgumentOwnershipV1::ByValue,
    ])
    .unwrap();
    let mut statements = Vec::new();
    let input = if matches!(shape, GlobalWriteExpressionShapeV1::LoadArithmetic) {
        statements.push(typed_assignment(
            4,
            A_U32,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(global_ranked_slice_place_v1())),
        ));
        4
    } else {
        3
    };
    let stored = if matches!(shape, GlobalWriteExpressionShapeV1::Parameter) {
        input
    } else {
        statements.push(typed_assignment(
            5,
            A_U32,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: typed_operand(input, A_U32),
                right: typed_constant(A_U32, 7, 4),
            },
        ));
        5
    };
    statements.push(global_ranked_slice_store_v1(stored));
    let guard = block(
        if dead { 201 } else { 203 },
        vec![
            typed_assignment(
                8,
                A_U64,
                SemanticRvalueKindV1::Use(typed_constant(A_U64, 0, 8)),
            ),
            typed_assignment(
                7,
                A_U64,
                SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::PointerMetadata,
                    operand: typed_operand(1, G_POINTER),
                },
            ),
            typed_assignment(
                9,
                A_BOOL,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: typed_operand(8, A_U64),
                    right: typed_operand(7, A_U64),
                },
            ),
            // This unused expression does not stand in for the dynamic guard.
            // The literal predecessor/selector supplies actual CFG optimization.
            typed_assignment(
                10,
                A_U32,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left: typed_constant(A_U32, 11, 4),
                    right: typed_constant(A_U32, 13, 4),
                },
            ),
        ],
        SemanticTerminatorKindV1::Assert {
            condition: typed_operand(9, A_BOOL),
            expected: true,
            message: SemanticAssertMessageV1::BoundsCheck {
                length: typed_operand(7, A_U64),
                index: typed_operand(8, A_U64),
            },
            target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, if dead { 3 } else { 1 }),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
    );
    let blocks = if dead {
        vec![
            guard,
            block(
                202,
                vec![global_ranked_slice_store_v1(3)],
                SemanticTerminatorKindV1::Return,
            ),
            block(203, statements, SemanticTerminatorKindV1::Return),
            block(
                204,
                vec![typed_assignment(
                    6,
                    A_BOOL,
                    SemanticRvalueKindV1::Use(typed_constant(A_BOOL, 0, 1)),
                )],
                zero_switch(6, A_BOOL, 1, 2),
            ),
        ]
    } else {
        vec![
            block(
                201,
                vec![],
                assertion_terminator(typed_constant(A_BOOL, 1, 1), true, 2),
            ),
            block(202, statements, SemanticTerminatorKindV1::Return),
            guard,
        ]
    };
    let mut locals = vec![
        (A_UNIT, SemanticLocalRoleV1::Return),
        (G_POINTER, SemanticLocalRoleV1::Argument(0)),
        (G_POINTER, SemanticLocalRoleV1::Argument(1)),
        (A_U32, SemanticLocalRoleV1::Argument(2)),
        (A_U32, SemanticLocalRoleV1::Temporary),
        (A_U32, SemanticLocalRoleV1::Temporary),
        (A_BOOL, SemanticLocalRoleV1::Temporary),
        (A_U64, SemanticLocalRoleV1::Temporary),
        (A_U64, SemanticLocalRoleV1::Temporary),
        (A_BOOL, SemanticLocalRoleV1::Temporary),
        (A_U32, SemanticLocalRoleV1::Temporary),
    ];
    // Preserve the shared fixture's admitted type closure, as assertion_root does.
    for ty in [A_CHECKED, A_BOOL_PTR, A_ARRAY] {
        locals.push((ty, SemanticLocalRoleV1::Temporary));
    }
    let dimensions = SemanticWorkgroupDimensionsV1::new([lanes, 1, 1]).unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(237)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(238)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(239)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(240)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(241)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals
            .into_iter()
            .enumerate()
            .map(|(i, (ty, role))| local(100 + i as u8, ty, role))
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(A_NAME.as_bytes().to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(247)),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let ssa = assertion_ssa_functions(global_ranked_slice_types_v1(), vec![function]);
    materialize_ranked_fixture_v1(ssa, &[ranked_root_input_1d(A_NAME, 247, lanes)]).unwrap()
}

fn with_actual_dynamic_global_root_v1<T>(
    source: &ProductionPreRankedKirOwnerV1,
    profile: Profile,
    lanes: u32,
    body: impl FnOnce(
        &ProductionRankedRootProgramV1,
        &ProductionSourceOutputOccurrencesV1<'_, '_>,
        &mut Budget<'_>,
    ) -> Result<T, ProductionRankedProjectionErrorV1>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let mut result = None;
    with_actual(source, profile, |bound, checked, budget| {
        let original = &source.executable().module().functions[0]
            .body
            .as_ref()
            .unwrap();
        let output = &checked.owner().module().functions[0].body.as_ref().unwrap();
        assert!(matches!(original.blocks.len(), 4 | 5));
        assert_ne!(
            bound.canonical().canonical_bytes(),
            checked.owner().canonical().canonical_bytes()
        );
        for body in [original, output] {
            assert!(body.blocks.iter().any(|block| matches!(
                block.terminator,
                Some(fe2o3_kernel_ir::Terminator::ConditionalBranch { .. })
            )));
            let traps = body.blocks.iter().flat_map(|block| &block.operations).filter(|operation| {
                matches!(&operation.kind, fe2o3_kernel_ir::OperationKind::Call { callee, arguments }
                    if fe2o3_kernel_ir::AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments)
                        == Some(fe2o3_kernel_ir::AmdGpuDiagnosticOperation::Trap))
            }).count();
            assert_eq!(traps, 1);
        }
        assert!(
            original
                .blocks
                .iter()
                .map(|b| b.operations.len())
                .sum::<usize>()
                > output
                    .blocks
                    .iter()
                    .map(|b| b.operations.len())
                    .sum::<usize>()
        );
        let floor = budget.storage();
        result = Some(
            with_projected_checked_output_roots_v1(
                source,
                bound,
                checked,
                profile,
                &[ranked_root_input_1d(A_NAME, 247, lanes)],
                &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
                budget,
                |roots, view, budget| {
                    let [root] = roots else {
                        panic!("one real Global root")
                    };
                    assert!(root.all_kernel_checks_are_clean());
                    assert!(std::ptr::eq(view.source(), source));
                    assert!(std::ptr::eq(view.output(), checked.owner()));
                    let floor = budget.storage();
                    let result = body(root, view, budget);
                    assert_eq!(budget.storage(), floor);
                    result
                },
            )
            .map(|(roots, _)| {
                assert_eq!(roots.len(), 1);
            }),
        );
        assert_eq!(budget.storage(), floor);
    });
    result.unwrap()
}

#[test]
fn actual_global_ranked_batch_checks_parameter_arithmetic_and_load_derived_writes() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for shape in [
            GlobalWriteExpressionShapeV1::Parameter,
            GlobalWriteExpressionShapeV1::ParameterArithmetic,
            GlobalWriteExpressionShapeV1::LoadArithmetic,
        ] {
            let source = genuine_dynamic_global_source_v1(shape, 1);
            let source_bytes = source.executable().canonical().canonical_bytes().to_vec();
            with_actual_dynamic_global_root_v1(&source, profile, 1, |root, view, budget| {
                let before = budget.work();
                view.check_ranked_global_allocation_values(
                    ROOT,
                    ROOT,
                    &root.lowering,
                    &root.access_sources,
                    budget,
                )
                .map_err(|error| {
                    ProductionRankedProjectionErrorV1::CanonicalAssertions(
                        CanonicalAssertionErrorV1::Output(error),
                    )
                })?;
                assert!(budget.work() > before);
                assert!(!view.grants_authority());
                Ok(())
            })
            .unwrap();
            assert_eq!(
                source.executable().canonical().canonical_bytes(),
                source_bytes.as_slice()
            );
        }
    }
}

#[test]
fn actual_global_ranked_batch_accepts_wholly_omitted_dead_global_rows_without_claims() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let source =
            genuine_global_source_variant_v1(GlobalWriteExpressionShapeV1::LoadArithmetic, 1, true);
        with_actual_dynamic_global_root_v1(&source,profile,1,|root,view,budget| {
            assert_eq!(root.access_sources.len(),1);
            assert_eq!(root.access_sources[0].semantic_block(),1);
            for statement in [0,2] {
                assert!(matches!(view.global_access(ROOT,ROOT,2,Some(statement),0,budget).unwrap(),
                    fe2o3_lower_mir_kernel::ProductionSourceOutputGlobalAccessV1::OmittedUnreachable { .. }));
            }
            view.check_ranked_global_allocation_values(ROOT,ROOT,&root.lowering,&root.access_sources,budget)
                .map_err(|error|ProductionRankedProjectionErrorV1::CanonicalAssertions(CanonicalAssertionErrorV1::Output(error)))?;
            let candidate=changed_global_candidate_v1(root,2);
            let mut claims=root.access_sources.clone();
            let original=claims[0];
            let extra=ProductionRankedAccessSourceV1::new(
                2,
                Some(0),
                original.semantic_access_ordinal(),
                original.ranked_block(),
                u32::try_from(root.lowering.kernel().blocks()[original.ranked_block() as usize].operations().len()).unwrap(),
            );
            claims.push(extra);
            let floor=budget.storage();
            assert!(matches!(view.check_ranked_global_allocation_values(ROOT,ROOT,&candidate,&claims,budget),
                Err(ProductionSourceOutputErrorV1::Invalid("global ranked omitted source access claimed"))));
            assert_eq!(budget.storage(),floor);
            Ok(())
        }).unwrap();
    }
}

#[test]
fn actual_global_diagnostic_candidate_whole_query_has_exact_work_and_under_cleanup() {
    let function = assertion_root_with_access(
        vec![(A_UNIT, SemanticLocalRoleV1::Return)],
        vec![],
        vec![block(201, vec![], SemanticTerminatorKindV1::Return)],
        false,
    );
    let source = assertion_materialized_functions(assertion_types(), vec![function]);
    let module = source.executable().module();
    assert_eq!(module.functions.len(), 1);
    assert_eq!(module.kernels.len(), 1);
    let body = module.functions[0].body.as_ref().unwrap();
    assert!(body.parameters.is_empty());
    assert_eq!(body.blocks.len(), 1);
    assert!(body.blocks[0].parameters.is_empty());
    assert!(body.blocks[0].operations.is_empty());
    let kernel = fe2o3_pliron::ProductionRankedKernelV1::new(
        A_NAME,
        0,
        vec![fe2o3_pliron::ProductionRankedBlockV1::new(
            vec![],
            fe2o3_pliron::ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let candidate = fe2o3_pliron::compile_ranked_kernel_for_lowering_v1(
        fe2o3_pliron::ProductionConstructionV1::ranked_kernel(
            "empty_global_query_candidate",
            kernel,
        )
        .unwrap(),
        fe2o3_pliron::ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    // This is an independently verified diagnostic candidate, not a projected
    // production root. The mandatory projector's no-memory refusal is intact.
    with_global_view(&source, Profile::Gfx942, |view, inherited| {
        let name = A_NAME.len() + 1;
        // API floor/name/alias 4+4+name+3+2; inventory census5,
        // five allocations5, fill5, index/kernel rows3, name query1+name;
        // function2; SSA sizing12 and four prepaid allocate/reconcile7;
        // one ranked block1, one source terminator span3, final count1.
        let exact =
            (4 + 4 + name + 3 + 2) + (5 + 5 + 5 + 3 + 1 + name) + 2 + (12 + 4 * 7) + 1 + 3 + 1;
        let floor = inherited.storage();
        for repeats in [1, 2] {
            for under in [false, true] {
                let limit = 7 + repeats * exact - usize::from(under);
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, floor + 1_000_000);
                budget.charge_work(7).unwrap();
                budget.reserve_storage(floor).unwrap();
                for iteration in 0..repeats {
                    let result = view.check_ranked_global_allocation_values(
                        ROOT,
                        ROOT,
                        &candidate,
                        &[],
                        &mut budget,
                    );
                    assert_eq!(result.is_ok(), !under || iteration + 1 < repeats);
                    assert_eq!(budget.storage(), floor);
                }
                assert_eq!(budget.work(), limit);
                assert!(budget.peak_storage() > floor);
                if under {
                    assert!(budget.charge_work(1).is_err());
                    assert_eq!(budget.storage(), floor);
                }
                drop(budget);
                assert_eq!(work.failed_work(), under.then_some(7 + repeats * exact));
            }
        }
        assert!(!view.grants_authority());
    });
}

fn changed_global_candidate_v1(
    root: &ProductionRankedRootProgramV1,
    change: usize,
) -> ProductionRankedKernelLoweringInputV1 {
    let blocks = root
        .lowering
        .kernel()
        .blocks()
        .iter()
        .enumerate()
        .map(|(block_index, block)| {
            let mut operations = block.operations().to_vec();
            for operation in &mut operations {
                match operation {
                    ProductionRankedOperationV1::ViewInSpace {
                        allocation_origin, ..
                    } if change == 0 => *allocation_origin = 2,
                    ProductionRankedOperationV1::View {
                        allocation_origin, ..
                    } if change == 0 => *allocation_origin = 2,
                    ProductionRankedOperationV1::SemanticExpression {
                        expression,
                        numerical_contract,
                        ..
                    } if change == 0 => {
                        // Keep this LoadArithmetic candidate internally valid;
                        // the source/output query must reject its changed origin.
                        use fe2o3_pliron::ProductionSemanticExpressionV2 as Expression;
                        let Expression::Binary { lhs, .. } = expression else {
                            panic!("expected the fixture's load arithmetic");
                        };
                        let Expression::Load(load) = lhs.as_mut() else {
                            panic!("expected the fixture's actual read");
                        };
                        assert_eq!(load.allocation_origin, 1);
                        load.allocation_origin = 2;
                        *numerical_contract =
                            fe2o3_pliron::ProductionNumericalContractV2::exact_for_expression(
                                expression,
                            );
                    }
                    ProductionRankedOperationV1::SemanticExpression {
                        expression,
                        numerical_contract,
                        ..
                    } if change == 1 => {
                        *expression = fe2o3_pliron::ProductionSemanticExpressionV2::Constant {
                            scalar: fe2o3_pliron::ProductionSemanticScalarTypeV2::Integer {
                                signed: false,
                                bits: 32,
                            },
                            bits: 99,
                        };
                        *numerical_contract =
                            fe2o3_pliron::ProductionNumericalContractV2::exact_for_expression(
                                expression,
                            );
                    }
                    _ => {}
                }
            }
            if change == 2 && block_index == root.access_sources[0].ranked_block() as usize {
                operations
                    .push(operations[root.access_sources[0].ranked_operation() as usize].clone());
            }
            fe2o3_pliron::ProductionRankedBlockV1::new(operations, block.terminator().clone())
        })
        .collect();
    let kernel = fe2o3_pliron::ProductionRankedKernelV1::new(
        root.function_name(),
        root.lowering.kernel().argument_count(),
        blocks,
    )
    .unwrap();
    fe2o3_pliron::compile_ranked_kernel_for_lowering_v1(
        fe2o3_pliron::ProductionConstructionV1::ranked_kernel("changed_global_candidate", kernel)
            .unwrap(),
        fe2o3_pliron::ProductionSessionLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn actual_global_ranked_batch_rejects_wrong_argument_wrong_value_and_missing_duplicate_sources() {
    let source = genuine_dynamic_global_source_v1(GlobalWriteExpressionShapeV1::LoadArithmetic, 1);
    with_actual_dynamic_global_root_v1(&source, Profile::Gfx942, 1, |root, view, budget| {
        for change in [0, 1] {
            let candidate = changed_global_candidate_v1(root, change);
            let floor = budget.storage();
            let result = view.check_ranked_global_allocation_values(
                ROOT,
                ROOT,
                &candidate,
                &root.access_sources,
                budget,
            );
            assert!(result.is_err(), "hostile ranked candidate {change}");
            assert_eq!(budget.storage(), floor);
        }
        let mut duplicate = root.access_sources.clone();
        duplicate.push(duplicate[0]);
        for sources in [&[][..], &root.access_sources[..1], duplicate.as_slice()] {
            let floor = budget.storage();
            assert!(
                view.check_ranked_global_allocation_values(
                    ROOT,
                    ROOT,
                    &root.lowering,
                    sources,
                    budget
                )
                .is_err()
            );
            assert_eq!(budget.storage(), floor);
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn actual_global_ranked_batch_cannot_bypass_mandatory_race_rejection() {
    let source =
        genuine_dynamic_global_source_v1(GlobalWriteExpressionShapeV1::ParameterArithmetic, 64);
    let called = std::cell::Cell::new(false);
    let result = with_actual_dynamic_global_root_v1(&source, Profile::Gfx942, 64, |_, _, _| {
        called.set(true);
        Ok(())
    });
    assert!(
        matches!(result, Err(ProductionRankedProjectionErrorV1::Compile { error, .. })
        if matches!(*error, fe2o3_pliron::ProductionRankedCompileErrorV1::Session(fe2o3_pliron::ProductionSessionErrorV1::RankedRace(_))))
    );
    assert!(!called.get());
}

include!("source_output_bounds_cfg_v1_tests.rs");
include!("source_output_ordinary_write_values_v1_tests.rs");
include!("ordinary_helper_summary_v1_tests.rs");
